//! Bounded read-only PE reader. See Microsoft PE Format and VERSIONINFO resource.
//! Never loads a DLL, invokes an executable, or trusts offsets before checking bounds.
use super::{types::ExecutableCandidate, Result, ServiceError};
use crate::domain::protocol::ErrorCode;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

fn u16at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        b.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}
fn u32at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn read_at(file: &mut File, offset: u64, size: usize) -> std::io::Result<Vec<u8>> {
    let mut b = vec![0; size];
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(&mut b)?;
    Ok(b)
}
fn malformed() -> ServiceError {
    ServiceError(
        ErrorCode::PathInvalid,
        "不是受支持的 Windows PE 可执行文件，或文件损坏。",
    )
}
fn align(n: usize) -> usize {
    (n + 3) & !3
}
fn version_strings(b: &[u8], depth: usize, out: &mut Vec<(String, String)>) {
    if depth > 8 || out.len() > 128 {
        return;
    }
    let Some(length) = u16at(b, 0).map(usize::from) else {
        return;
    };
    if length < 6 || length > b.len() {
        return;
    }
    let b = &b[..length];
    let mut at = 6;
    let mut key = vec![];
    loop {
        let Some(c) = u16at(b, at) else {
            return;
        };
        at += 2;
        if c == 0 {
            break;
        }
        key.push(c);
        if key.len() > 256 {
            return;
        }
    }
    let key = String::from_utf16_lossy(&key);
    at = align(at);
    let words = usize::from(u16at(b, 2).unwrap_or(0));
    let bytes = if u16at(b, 4) == Some(1) {
        words.saturating_mul(2)
    } else {
        words
    };
    let Some(value) = b.get(at..at.saturating_add(bytes)) else {
        return;
    };
    if ["ProductName", "FileDescription", "CompanyName"].contains(&key.as_str())
        && u16at(b, 4) == Some(1)
    {
        let wide = value
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|c| *c != 0)
            .take(1024)
            .collect::<Vec<_>>();
        out.push((key, String::from_utf16_lossy(&wide)));
    }
    at = align(at.saturating_add(bytes));
    let mut children = 0;
    while at + 6 <= b.len() && children < 256 {
        let len = usize::from(u16at(b, at).unwrap_or(0));
        if len < 6 || at + len > b.len() {
            break;
        }
        version_strings(&b[at..at + len], depth + 1, out);
        at = align(at + len);
        children += 1;
    }
}
fn resource_data(b: &[u8], at: usize, depth: usize, budget: &mut usize) -> Option<(u32, usize)> {
    if depth > 3 || *budget == 0 {
        return None;
    }
    *budget -= 1;
    let count = usize::from(u16at(b, at + 12)?) + usize::from(u16at(b, at + 14)?);
    if count > 256 {
        return None;
    }
    for i in 0..count {
        let entry = at.checked_add(16 + i * 8)?;
        let name = u32at(b, entry)?;
        if depth == 0 && name != 16 {
            continue;
        }
        let offset = u32at(b, entry + 4)?;
        let p = (offset & 0x7fff_ffff) as usize;
        if offset & 0x8000_0000 != 0 {
            if let Some(data) = resource_data(b, p, depth + 1, budget) {
                return Some(data);
            }
        } else {
            return Some((u32at(b, p)?, u32at(b, p + 4)? as usize));
        }
    }
    None
}
pub fn inspect(path: &Path) -> Result<ExecutableCandidate> {
    let mut f = File::open(path)
        .map_err(|_| ServiceError(ErrorCode::PermissionDenied, "无法读取可执行文件。"))?;
    let metadata = f.metadata().map_err(|_| malformed())?;
    let len = metadata.len();
    let dos = read_at(&mut f, 0, 64).map_err(|_| malformed())?;
    if dos.get(..2) != Some(b"MZ") {
        return Err(malformed());
    }
    let pe = u64::from(u32at(&dos, 60).ok_or_else(malformed)?);
    if pe > 1024 * 1024 || pe + 24 > len {
        return Err(malformed());
    }
    let coff = read_at(&mut f, pe, 24).map_err(|_| malformed())?;
    let sections = usize::from(u16at(&coff, 6).ok_or_else(malformed)?);
    let optional = usize::from(u16at(&coff, 20).ok_or_else(malformed)?);
    let flags = u16at(&coff, 22).ok_or_else(malformed)?;
    if coff.get(..4) != Some(b"PE\0\0")
        || !matches!(u16at(&coff, 4), Some(0x14c | 0x8664))
        || !(1..=96).contains(&sections)
        || !(96..=4096).contains(&optional)
        || flags & 2 == 0
        || flags & 0x2000 != 0
    {
        return Err(malformed());
    }
    let header = read_at(&mut f, pe + 24, optional + 40 * sections).map_err(|_| malformed())?;
    let directory = match u16at(&header, 0) {
        Some(0x10b) => 96,
        Some(0x20b) => 112,
        _ => return Err(malformed()),
    };
    let rva = u32at(&header, directory + 16).unwrap_or(0);
    let size = u32at(&header, directory + 20).unwrap_or(0) as usize;
    let section_map = |rva: u32, size: usize| -> Option<u64> {
        for n in 0..sections {
            let at = optional + n * 40;
            let va = u32at(&header, at + 12)?;
            let raw = u32at(&header, at + 16)?;
            let ptr = u32at(&header, at + 20)?;
            if rva >= va && u64::from(rva - va) + size as u64 <= u64::from(raw) {
                let pos = u64::from(ptr) + u64::from(rva - va);
                if pos + size as u64 <= len {
                    return Some(pos);
                }
            }
        }
        None
    };
    let mut strings = vec![];
    if rva != 0 && (16..=4 * 1024 * 1024).contains(&size) {
        if let Some(pos) = section_map(rva, size) {
            if let Ok(resource) = read_at(&mut f, pos, size) {
                if let Some((data_rva, data_size)) = resource_data(&resource, 0, 0, &mut 256) {
                    if data_size <= 1024 * 1024 {
                        if let Some(pos) = section_map(data_rva, data_size) {
                            if let Ok(data) = read_at(&mut f, pos, data_size) {
                                version_strings(&data, 0, &mut strings);
                            }
                        }
                    }
                }
            }
        }
    }
    // Incremental fingerprint: size/mtime plus SHA-256 of first/last 64 KiB and PE headers.
    let mut hash = Sha256::new();
    hash.update(len.to_le_bytes());
    if let Ok(m) = metadata.modified() {
        if let Ok(t) = m.duration_since(std::time::UNIX_EPOCH) {
            hash.update(t.as_nanos().to_le_bytes());
        }
    }
    hash.update(read_at(&mut f, 0, len.min(65536) as usize).map_err(|_| malformed())?);
    if len > 65536 {
        hash.update(read_at(&mut f, len - 65536, 65536).map_err(|_| malformed())?);
    }
    hash.update(&header);
    let get = |key: &str| {
        strings
            .iter()
            .find(|(k, v)| k == key && !v.trim().is_empty())
            .map(|(_, v)| v.trim().to_owned())
    };
    let mut title_evidence = vec![];
    if let Some(folder) = path.parent().and_then(Path::file_name) {
        title_evidence.push(super::types::TitleEvidence {
            source: "folder_name".into(),
            value: folder.to_string_lossy().into_owned(),
            weight: 60,
        });
    }
    if let Some(file) = path.file_stem() {
        title_evidence.push(super::types::TitleEvidence {
            source: "file_name".into(),
            value: file.to_string_lossy().into_owned(),
            weight: 35,
        });
    }
    for (key, weight) in [("ProductName", 95), ("FileDescription", 75)] {
        if let Some(value) = get(key) {
            title_evidence.push(super::types::TitleEvidence {
                source: format!("pe.{key}"),
                value,
                weight,
            });
        }
    }
    Ok(ExecutableCandidate {
        path: super::path_text(path)?,
        product_name: get("ProductName"),
        file_description: get("FileDescription"),
        company_name: get("CompanyName"),
        fingerprint: format!("{:x}", hash.finalize()),
        title_evidence,
    })
}

#[cfg(test)]
pub fn fixture() -> Vec<u8> {
    let mut b = vec![0u8; 512];
    b[..2].copy_from_slice(b"MZ");
    b[60..64].copy_from_slice(&64u32.to_le_bytes());
    b[64..68].copy_from_slice(b"PE\0\0");
    b[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
    b[70..72].copy_from_slice(&1u16.to_le_bytes());
    b[84..86].copy_from_slice(&240u16.to_le_bytes());
    b[86..88].copy_from_slice(&2u16.to_le_bytes());
    b[88..90].copy_from_slice(&0x20bu16.to_le_bytes());
    b
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pe_resource_rva_mapping_reads_utf16_version_and_rejects_bad_offsets() {
        let root = std::env::temp_dir().join(super::super::id());
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("story.exe");
        let mut node = vec![0; 6];
        for c in "ProductName".encode_utf16().chain([0]) {
            node.extend(c.to_le_bytes());
        }
        node.resize(align(node.len()), 0);
        let value = "星空物语".encode_utf16().chain([0]).collect::<Vec<_>>();
        node[2..4].copy_from_slice(&(value.len() as u16).to_le_bytes());
        node[4..6].copy_from_slice(&1u16.to_le_bytes());
        for c in value {
            node.extend(c.to_le_bytes());
        }
        let length = node.len() as u16;
        node[..2].copy_from_slice(&length.to_le_bytes());
        let mut b = fixture();
        b.resize(1536, 0);
        b[216..220].copy_from_slice(&0x1000u32.to_le_bytes());
        b[220..224].copy_from_slice(&1024u32.to_le_bytes());
        b[340..344].copy_from_slice(&0x1000u32.to_le_bytes());
        b[344..348].copy_from_slice(&1024u32.to_le_bytes());
        b[348..352].copy_from_slice(&512u32.to_le_bytes());
        for at in [512, 544, 576] {
            b[at + 14..at + 16].copy_from_slice(&1u16.to_le_bytes());
        }
        b[528..532].copy_from_slice(&16u32.to_le_bytes());
        b[532..536].copy_from_slice(&0x80000020u32.to_le_bytes());
        b[564..568].copy_from_slice(&0x80000040u32.to_le_bytes());
        b[596..600].copy_from_slice(&96u32.to_le_bytes());
        b[608..612].copy_from_slice(&0x1100u32.to_le_bytes());
        b[612..616].copy_from_slice(&(node.len() as u32).to_le_bytes());
        b[768..768 + node.len()].copy_from_slice(&node);
        std::fs::write(&file, &b).unwrap();
        let c = inspect(&file).unwrap();
        assert_eq!(c.product_name.as_deref(), Some("星空物语"));
        assert!(c
            .title_evidence
            .iter()
            .any(|e| e.source == "pe.ProductName" && e.weight == 95));
        b[608..612].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&file, &b).unwrap();
        assert!(inspect(&file).unwrap().product_name.is_none());
        b[60..64].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&file, &b).unwrap();
        assert!(inspect(&file).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bounded_version_tree_reads_strings() {
        fn node(key: &str, value: &str) -> Vec<u8> {
            let mut b = vec![0; 6];
            for c in key.encode_utf16().chain([0]) {
                b.extend(c.to_le_bytes());
            }
            b.resize(align(b.len()), 0);
            let wide = value.encode_utf16().chain([0]).collect::<Vec<_>>();
            b[2..4].copy_from_slice(&(wide.len() as u16).to_le_bytes());
            b[4..6].copy_from_slice(&1u16.to_le_bytes());
            for c in wide {
                b.extend(c.to_le_bytes());
            }
            let len = b.len() as u16;
            b[..2].copy_from_slice(&len.to_le_bytes());
            b
        }
        let b = node("ProductName", "星空物语");
        let mut out = vec![];
        version_strings(&b, 0, &mut out);
        assert_eq!(out, vec![("ProductName".into(), "星空物语".into())]);
        let mut invalid = b;
        invalid[..2].copy_from_slice(&65535u16.to_le_bytes());
        version_strings(&invalid, 0, &mut out);
        assert_eq!(out.len(), 1);
    }
    #[test]
    fn malformed_resource_offsets_never_escape_buffer() {
        for len in 0..1024 {
            let b = vec![0xff; len];
            assert!(resource_data(&b, 0, 0, &mut 256).is_none());
            let mut strings = vec![];
            version_strings(&b, 0, &mut strings);
        }
    }
}
