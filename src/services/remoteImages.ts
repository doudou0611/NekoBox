import { api, coverSource, desktop } from '../stores/library';
const requests = new Map<string, Promise<string>>();
export function remoteImage(url: string, avatar = false): Promise<string> {
  if (
    !desktop ||
    !/^https?:/i.test(url) ||
    /^https?:\/\/(cover|asset)\.localhost/i.test(url)
  )
    return Promise.resolve(url);
  const key = `${avatar}:${url}`;
  let request = requests.get(key);
  if (!request) {
    request = api('cache_remote_image', { url, avatar })
      .then(coverSource)
      .catch((error) => {
        requests.delete(key);
        throw error;
      });
    requests.set(key, request);
  }
  return request;
}
export async function localizeAvatar<
  T extends { profile: { avatar_url: string | null } | null },
>(account: T): Promise<T> {
  const url = account.profile?.avatar_url;
  if (url && account.profile)
    account.profile.avatar_url = await remoteImage(url, true).catch(() => null);
  return account;
}
