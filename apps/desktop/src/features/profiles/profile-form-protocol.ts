import type { ProfileProtocol } from "@/ipc/bindings";
import { CONFIG_TYPES } from "./profile-constants";
import { clean } from "./profile-form-text";
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
        cipher: clean(options.vmessCipher),
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
        flow: clean(options.flow),
        encryption: clean(options.vlessEncryption),
      };
    case CONFIG_TYPES.Trojan:
      return { kind: "trojan", server, password: parsed.password ?? "" };
    case CONFIG_TYPES.Hysteria2:
      return {
        kind: "hysteria2",
        server,
        password: parsed.password ?? "",
        portHops: clean(options.portHops),
        obfuscationPassword: clean(options.obfuscationPassword),
      };
    case CONFIG_TYPES.TUIC:
      return {
        kind: "tuic",
        server,
        uuid: parsed.username ?? "",
        password: parsed.password ?? "",
        congestionControl: clean(options.congestionControl),
      };
    case CONFIG_TYPES.WireGuard:
      return {
        kind: "wireGuard",
        server,
        privateKey: parsed.password ?? "",
        peerPublicKey: clean(options.wireGuardPeerPublicKey),
        presharedKey: clean(options.wireGuardPresharedKey),
        interfaceAddress: clean(options.wireGuardInterfaceAddress),
        allowedIps: clean(options.wireGuardAllowedIps),
        reserved: clean(options.wireGuardReserved),
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
        congestionControl: clean(options.congestionControl),
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
