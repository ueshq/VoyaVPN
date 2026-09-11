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
    realitySpiderX: clean(parsed.spiderX),
    mldsa65Verify: clean(parsed.mldsa65Verify),
    certificatePem: clean(parsed.cert),
    certificateSha256: splitList(parsed.certSha),
    echConfig: splitList(parsed.echConfigList),
    finalMask: clean(parsed.finalmask),
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
        spiderX: tls.realitySpiderX ?? "",
        mldsa65Verify: tls.mldsa65Verify ?? "",
        cert: tls.certificatePem ?? "",
        certSha: tls.certificateSha256.join(","),
        echConfigList: tls.echConfig.join(","),
        finalmask: tls.finalMask ?? "",
      }
    : {};
}
