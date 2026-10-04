//! Read-only official search smoke; isolated temporary repository, no account writes.
use galgame_manager_lib::{
    backend::{self, Backend},
    domain::requests::SearchMetadataRequest,
};
fn main() {
    let root = std::env::temp_dir().join(format!("gm-metadata-probe-{}", backend::id()));
    let mut failures = 0;
    let mut matches = vec![];
    {
        let b = Backend::open(root.join("data")).expect("isolated probe repository");
        for provider in ["hikarinagi", "bangumi", "vndb"] {
            let query = SearchMetadataRequest {
                manual: false,
                batch_id: None,
                query: "AIR".into(),
                providers: vec![provider.into()],
                cache: false,
            };
            match backend::metadata_sources::search(&b, &query) {
                Ok(mut items) => {
                    items.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
                    if let Some(best) = items.first().filter(|best| {
                        best.confidence >= 0.95
                            && items
                                .get(1)
                                .is_none_or(|next| best.confidence - next.confidence >= 0.05)
                    }) {
                        matches.push((
                            provider.to_owned(),
                            best.remote_id.clone(),
                            best.title.clone(),
                        ));
                    }
                    println!(
                        "{provider}: official search parsed, {} candidates",
                        items.len()
                    );
                    if items.is_empty() {
                        failures += 1;
                    }
                }
                Err(e) => {
                    println!("{provider}: {:?}, {}", e.0, e.1);
                    failures += 1;
                }
            }
        }
        if std::env::args().any(|arg| arg == "--prepare") {
            matches.sort_by_key(|(p, _, _)| if p == "vndb" { 0 } else { 1 });
            if let Some((provider, remote_id, title)) = matches.into_iter().next() {
                let directory = root.join("game");
                std::fs::create_dir_all(&directory).unwrap();
                let game_id = b
                    .database()
                    .unwrap()
                    .import_installation(
                        &backend::path_text(&directory).unwrap(),
                        "game",
                        None,
                        galgame_manager_lib::domain::protocol::InstallSource::Manual,
                        &[],
                        "",
                    )
                    .unwrap();
                let request = galgame_manager_lib::domain::requests::ConfirmMetadataMatchRequest {
                    manual: false,
                    title_hint: Some(title),
                    game_id: game_id.clone(),
                    provider,
                    remote_id,
                };
                match backend::translation::confirm(&b, &request) {
                    Ok(result) => {
                        let detail = b.database().unwrap().get_game(&game_id).unwrap();
                        println!(
                            "full metadata: {} fields; {}",
                            detail.metadata.len(),
                            result.cover_message.unwrap_or_default()
                        );
                        if let Some(message) = result.supplementation_message {
                            println!("supplementation: {message}");
                        }
                    }
                    Err(e) => {
                        println!("full metadata: {:?}, {}", e.0, e.1);
                        failures += 1;
                    }
                }
            } else {
                println!("full metadata: no unique candidate, requires manual matching");
                failures += 1;
            }
        }
    }
    let _ = std::fs::remove_dir_all(root);
    if failures > 0 {
        std::process::exit(1);
    }
}
