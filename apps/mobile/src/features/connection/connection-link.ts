/** Exact actions only: no configuration, credentials or arbitrary command names from a URL. */
export function connectionLinkAction(url: string | null | undefined): "connect" | "disconnect" | null {
  if (url === "voyavpn://connect") return "connect";
  if (url === "voyavpn://disconnect") return "disconnect";
  return null;
}

