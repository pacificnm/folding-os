const PASSKEY_XML_RE = /(?:passkey|account-token)[^>]*\bv\s*=\s*["']([^"']+)["']/i;
const PASSKEY_MIN_LEN = 8;
const PASSKEY_MAX_LEN = 128;

function isValidFahPasskey(value: string): boolean {
  if (!value || value.length < PASSKEY_MIN_LEN || value.length > PASSKEY_MAX_LEN) {
    return false;
  }
  return /^[0-9a-zA-Z+/=_-]+$/.test(value);
}

export function normalizePasskeyInput(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) {
    return "";
  }

  const xmlMatch = trimmed.match(PASSKEY_XML_RE);
  if (xmlMatch?.[1]) {
    const value = xmlMatch[1].trim();
    if (isValidFahPasskey(value)) {
      return value;
    }
    throw new Error(formatPasskeyError(value.length));
  }

  if (isValidFahPasskey(trimmed)) {
    return trimmed;
  }

  throw new Error(formatPasskeyError(trimmed.length));
}

export function formatPasskeyError(length: number): string {
  return `Passkey must be ${PASSKEY_MIN_LEN} through ${PASSKEY_MAX_LEN} letters, digits, or base64/base64url characters (+/=-_); got ${length} characters. Paste the exact value from config.xml or your FAH passkey email.`;
}
