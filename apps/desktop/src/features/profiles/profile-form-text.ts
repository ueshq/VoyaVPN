export function clean(value: string | null | undefined) {
  const trimmed = value?.trim() ?? "";
  return trimmed || null;
}

export function splitList(value: string | null | undefined) {
  return (value ?? "")
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter(Boolean);
}
