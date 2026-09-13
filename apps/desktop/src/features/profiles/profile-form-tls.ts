import type { TlsSettings } from "@/ipc/bindings";
import { clean, splitList } from "./profile-form-text";
import type { ParsedProfileFormValues } from "./profile-form-schema";
export function formTls(parsed: ParsedProfileFormValues): TlsSettings | null {
  if (parsed.streamSecurity !== "tls" && parsed.streamSecurity !== "reality")
    return null;
  return {
    mode: parsed.streamSecurity,
    serverName: clean(parsed.sni),
    alpn: splitList(parsed.alpn),
    realityPublicKey: clean(parsed.publicKey),
    realityShortId: clean(parsed.shortId),
    certificatePem: clean(parsed.cert),
    echConfig: splitList(parsed.echConfigList),
  };
}

export function tlsToFormFields(tls: TlsSettings | null) {
  return tls
    ? {
        streamSecurity: tls.mode,
        sni: tls.serverName ?? "",
        alpn: tls.alpn.join(","),
        publicKey: tls.realityPublicKey ?? "",
        shortId: tls.realityShortId ?? "",
        cert: tls.certificatePem ?? "",
        echConfigList: tls.echConfig.join(","),
      }
    : {};
}
