import { describe, expect, it } from 'vitest';
import {
  changedMetadata,
  launchEnvironment,
  mergeMetadataDraft,
  metadataValues,
} from './detailWorkspace';
describe('detail workspace data', () => {
  it('submits changed fields with their old revision, without making scraped fields manual', () => {
    const base = metadataValues({
      title: '原名',
      description: '刮削简介',
      source_rating: 8.5,
    });
    const draft = { ...base, description: '手动简介' };
    expect(changedMetadata(draft, base)).toEqual({
      changes: { description: '手动简介' },
      expected: { description: '刮削简介' },
    });
  });
  it('fills newly scraped values while protecting dirty text and its optimistic revision', () => {
    const base = metadataValues({ title: '原名', description: '旧简介' });
    const draft = { ...base, description: '编辑中' };
    mergeMetadataDraft(
      draft,
      base,
      metadataValues({
        title: '新名称',
        description: '新简介',
        source_rating: 9,
      }),
    );
    expect(draft).toMatchObject({
      title: '新名称',
      description: '编辑中',
      source_rating: 9,
    });
    expect(base.description).toBe('旧简介');
    expect(changedMetadata(draft, base).expected).toEqual({
      description: '旧简介',
    });
  });
  it('represents explicit clearing and rating zero without guessing a fallback', () => {
    const base = metadataValues({ developer: '开发商', source_rating: 8 });
    expect(
      changedMetadata({ ...base, developer: ' ', source_rating: 0 }, base)
        .changes,
    ).toEqual({ developer: null, source_rating: 0 });
  });
  it('keeps environment values literal and rejects duplicates and invalid names', () => {
    expect(
      launchEnvironment([
        { key: ' PATH ', value: 'C:\\My Game' },
        { key: '', value: '' },
      ]),
    ).toEqual({ PATH: 'C:\\My Game' });
    expect(() =>
      launchEnvironment([
        { key: 'X', value: '1' },
        { key: 'X', value: '2' },
      ]),
    ).toThrow('重复');
    expect(() => launchEnvironment([{ key: 'A=B', value: '' }])).toThrow(
      '有效名称',
    );
    expect(
      Object.keys(launchEnvironment([{ key: '__proto__', value: 'literal' }])),
    ).toEqual(['__proto__']);
  });
});
