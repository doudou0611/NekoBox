import { describe, expect, it } from 'vitest';
import type { MetadataCandidate } from '../types/domain';
import { automaticCandidate, automaticOwnedCandidate } from './metadataScraper';
function candidate(
  overrides: Partial<MetadataCandidate> = {},
): MetadataCandidate {
  return {
    provider: 'bangumi',
    remote_id: '123',
    title: '作品',
    subtitle: null,
    cover_url: null,
    has_chinese_description: false,
    confidence: 1,
    matched_fields: ['title'],
    explanation: '',
    fetched_at: '',
    cached: false,
    ...overrides,
  };
}
describe('automatic identity matching', () => {
  it.each(['中文作品', 'English Title', '彼女との物語'])(
    'accepts a unique identity regardless of language: %s',
    (title) => {
      expect(automaticCandidate([candidate({ title })])?.title).toBe(title);
    },
  );
  it('accepts an identity without a cover so later sources can supplement', () => {
    expect(automaticCandidate([candidate()])?.remote_id).toBe('123');
  });
  it('requires confidence and separation from competing works', () => {
    expect(automaticCandidate([])).toBeNull();
    expect(automaticCandidate([candidate({ confidence: 0.9 })])).toBeNull();
    expect(
      automaticCandidate([
        candidate(),
        candidate({ remote_id: '124', confidence: 0.99 }),
      ]),
    ).toBeNull();
  });
  it('chooses by identity confidence even when another result has Chinese prose', () => {
    expect(
      automaticCandidate([
        candidate({ title: 'English Title' }),
        candidate({
          remote_id: '124',
          confidence: 0.8,
          has_chinese_description: true,
        }),
      ])?.remote_id,
    ).toBe('123');
  });
});

describe('official owned-game title matching', () => {
  it('automatically selects the one exact title even when provider scores tie', () => {
    expect(
      automaticOwnedCandidate('作品', [
        candidate({ confidence: 0.55 }),
        candidate({ title: '作品 另一版本', remote_id: '124', confidence: 1 }),
      ])?.remote_id,
    ).toBe('123');
  });
  it('accepts exact aliases, width and punctuation variations', () => {
    expect(
      automaticOwnedCandidate('ATRI：My Dear Moments', [
        candidate({
          title: '亚托莉',
          subtitle: 'ＡＴＲＩ - My Dear Moments',
          confidence: 0.9,
        }),
      ])?.remote_id,
    ).toBe('123');
  });
  it('deduplicates repeated identities and keeps genuinely different same-title records unmatched', () => {
    expect(
      automaticOwnedCandidate('作品', [candidate(), candidate()])?.remote_id,
    ).toBe('123');
    expect(
      automaticOwnedCandidate('作品', [
        candidate(),
        candidate({ remote_id: '124' }),
      ]),
    ).toBeNull();
    expect(automaticOwnedCandidate('...', [candidate()])).toBeNull();
  });
});
