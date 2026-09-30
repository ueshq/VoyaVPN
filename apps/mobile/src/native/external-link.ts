import { Linking } from "react-native";

/**
 * Opens a web page in the system browser. It resolves to `false` instead of
 * throwing when nothing can open the link, so a caller needs no error path for
 * a tap on a link.
 */
export async function openExternalLink(url: string): Promise<boolean> {
  try {
    await Linking.openURL(url);
    return true;
  } catch {
    return false;
  }
}
