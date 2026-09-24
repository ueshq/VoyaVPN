import { getErrorMessage } from "./error";

export type OperationalRedactionOptions = {
  redactedUrl?: string;
  redactedValue?: string;
};

const DEFAULT_REDACTED_URL = "[redacted URL]";
const DEFAULT_REDACTED_VALUE = "[redacted]";

const SENSITIVE_ASSIGNMENT_PATTERN =
  /(^|[\s,;({[])(proxyUrl|proxy_url|proxy|HTTP_PROXY|HTTPS_PROXY|user|username|userName|user_name|UserName|password|passwd|pass|token|access_token|refresh_token|authorization|auth|secret)(\s*[=:]\s*)(?:Bearer\s+)?[^\s<>"')\]]+/gi;
const SHARE_LINK_PATTERN = /\b(vless|vmess|trojan|ss|ssr|hysteria2|hy2|tuic|wireguard|wg|socks|socks5|anytls|naive|naive\+https):\/\/[^\s<>"')\]]+/gi;
const URL_PATTERN = /\bhttps?:\/\/[^\s<>"')\]]+/gi;

export function redactOperationalError(error: unknown, options?: OperationalRedactionOptions) {
  return redactOperationalMessage(getErrorMessage(error), options);
}

export function redactOperationalMessage(
  message: string,
  options: OperationalRedactionOptions = {},
) {
  const redactedUrl = options.redactedUrl ?? DEFAULT_REDACTED_URL;
  const redactedValue = options.redactedValue ?? DEFAULT_REDACTED_VALUE;

  return message
    .replace(
      SENSITIVE_ASSIGNMENT_PATTERN,
      (_match, prefix: string, key: string, separator: string) =>
        `${prefix}${key}${separator}${redactedValue}`,
    )
    .replace(/("(?:password|passwd|token|access_token|refresh_token|authorization|secret|uuid|privateKey|private_key)"\s*:\s*)"(?:\\.|[^"\\])*"/gi, (_match, prefix: string) => `${prefix}"${redactedValue}"`)
    .replace(/\bBearer\s+[A-Za-z0-9._~+/=-]+/gi, `Bearer ${redactedValue}`)
    .replace(SHARE_LINK_PATTERN, redactedValue)
    .replace(URL_PATTERN, redactedUrl);
}
