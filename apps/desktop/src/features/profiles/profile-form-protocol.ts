import type { ProfileProtocol } from "@/ipc/bindings";
import { trimToNull } from "@voya/utils/text";
import { CONFIG_TYPES } from "./profile-constants";
import type { ParsedProfileFormValues } from "./profile-form-schema";
export function formProtocol(parsed: ParsedProfileFormValues): ProfileProtocol {
  const options = parsed.protocolOptions;
  const server = { address: parsed.address, port: parsed.port };
  switch (parsed.configType) {
    case CONFIG_TYPES.VMess:
      return {
        kind: "vmess",
        server,
        uuid: parsed.password ?? "",
        cipher: trimToNull(options.vmessCipher),
      };
    case CONFIG_TYPES.Shadowsocks:
      return {
        kind: "shadowsocks",
        server,
        password: parsed.password ?? "",
        method: options.method ?? "",
        udpOverTcp: options.udpOverTcp === true,
      };
    case CONFIG_TYPES.SOCKS:
      return {
        kind: "socks",
        server,
        username: parsed.username ?? "",
        password: parsed.password ?? "",
      };
    case CONFIG_TYPES.VLESS:
      return {
        kind: "vless",
        server,
        uuid: parsed.password ?? "",
        flow: trimToNull(options.flow),
        encryption: trimToNull(options.vlessEncryption),
      };
    case CONFIG_TYPES.Trojan:
      return { kind: "trojan", server, password: parsed.password ?? "" };
    case CONFIG_TYPES.Hysteria2:
      return {
        kind: "hysteria2",
        server,
        password: parsed.password ?? "",
        portHops: trimToNull(options.portHops),
        obfuscationPassword: trimToNull(options.obfuscationPassword),
      };
    case CONFIG_TYPES.TUIC:
      return {
        kind: "tuic",
        server,
        uuid: parsed.username ?? "",
        password: parsed.password ?? "",
        congestionControl: trimToNull(options.congestionControl),
      };
    case CONFIG_TYPES.WireGuard:
      return {
        kind: "wireGuard",
        server,
        privateKey: parsed.password ?? "",
        peerPublicKey: trimToNull(options.wireGuardPeerPublicKey),
        presharedKey: trimToNull(options.wireGuardPresharedKey),
        interfaceAddress: trimToNull(options.wireGuardInterfaceAddress),
        allowedIps: trimToNull(options.wireGuardAllowedIps),
        reserved: trimToNull(options.wireGuardReserved),
        mtu: options.wireGuardMtu ?? null,
      };
    case CONFIG_TYPES.HTTP:
      return {
        kind: "http",
        server,
        username: parsed.username ?? "",
        password: parsed.password ?? "",
      };
    case CONFIG_TYPES.Anytls:
      return { kind: "anytls", server, password: parsed.password ?? "" };
    case CONFIG_TYPES.Naive:
      return {
        kind: "naive",
        server,
        username: parsed.username ?? "",
        password: parsed.password ?? "",
        quic: options.naiveQuic === true,
        congestionControl: trimToNull(options.congestionControl),
        insecureConcurrency: options.insecureConcurrency ?? null,
        udpOverTcp: options.udpOverTcp === true,
      };
  }
}

export function protocolToFormFields(protocol: ProfileProtocol) {
  switch (protocol.kind) {
    case "vmess":
      return { password: protocol.uuid };
    case "shadowsocks":
    case "trojan":
    case "hysteria2":
    case "anytls":
      return { password: protocol.password };
    case "socks":
    case "http":
    case "naive":
      return { password: protocol.password, username: protocol.username };
    case "vless":
      return { password: protocol.uuid };
    case "tuic":
      return { password: protocol.password, username: protocol.uuid };
    case "wireGuard":
      return { password: protocol.privateKey };
  }
}

export function protocolToFormOptions(protocol: ProfileProtocol) {
  switch (protocol.kind) {
    case "vmess":
      return { vmessCipher: protocol.cipher };
    case "shadowsocks":
      return { method: protocol.method, udpOverTcp: protocol.udpOverTcp };
    case "vless":
      return { flow: protocol.flow, vlessEncryption: protocol.encryption };
    case "hysteria2":
      return {
        portHops: protocol.portHops,
        obfuscationPassword: protocol.obfuscationPassword,
      };
    case "tuic":
      return { congestionControl: protocol.congestionControl };
    case "wireGuard":
      return {
        wireGuardPeerPublicKey: protocol.peerPublicKey,
        wireGuardPresharedKey: protocol.presharedKey,
        wireGuardInterfaceAddress: protocol.interfaceAddress,
        wireGuardAllowedIps: protocol.allowedIps,
        wireGuardReserved: protocol.reserved,
        wireGuardMtu: protocol.mtu,
      };
    case "naive":
      return {
        naiveQuic: protocol.quic,
        congestionControl: protocol.congestionControl,
        insecureConcurrency: protocol.insecureConcurrency,
        udpOverTcp: protocol.udpOverTcp,
      };
    default:
      return {};
  }
}
