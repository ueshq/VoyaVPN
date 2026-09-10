export function nullableText(value: string): string | null {
  return value.trim() ? value : null;
}
