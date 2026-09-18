/**
 * Which caller addresses the probe may connect back to.
 *
 * `CF-Connecting-IP` is set by Cloudflare's edge and is always the caller's
 * public address, but the check is repeated here so that no header, proxy or
 * test double can ever point the probe at a private network, loopback or
 * shared address space.
 */

export type AddressFamily = "ipv4" | "ipv6";

export function addressFamily(address: string): AddressFamily | null {
  if (parseIpv4(address)) return "ipv4";
  if (parseIpv6(address)) return "ipv6";
  return null;
}

/** A globally routable unicast address the probe may dial. */
export function isPublicAddress(address: string): boolean {
  const ipv4 = parseIpv4(address);
  if (ipv4) return isPublicIpv4(ipv4);
  const ipv6 = parseIpv6(address);
  if (!ipv6) return false;
  // IPv4-mapped (::ffff:0:0/96) is judged by the embedded IPv4 address.
  if (ipv6.slice(0, 5).every((group) => group === 0) && ipv6[5] === 0xffff) {
    return isPublicIpv4([ipv6[6] >> 8, ipv6[6] & 0xff, ipv6[7] >> 8, ipv6[7] & 0xff]);
  }
  // Only global unicast (2000::/3) is public.
  if ((ipv6[0] & 0xe000) !== 0x2000) return false;
  // 2001::/32 is Teredo and 2001:db8::/32 documentation; 2002::/16 is 6to4.
  if (ipv6[0] === 0x2001) return ipv6[1] !== 0 && ipv6[1] !== 0x0db8;
  return ipv6[0] !== 0x2002;
}

function isPublicIpv4([a, b]: number[]): boolean {
  if (a === 0 || a === 10 || a === 127 || a >= 224) return false;
  if (a === 100 && b >= 64 && b <= 127) return false;
  if (a === 169 && b === 254) return false;
  if (a === 172 && b >= 16 && b <= 31) return false;
  if (a === 192 && b === 168) return false;
  return true;
}

function parseIpv4(address: string): number[] | null {
  const parts = address.split(".");
  if (parts.length !== 4) return null;
  const octets = parts.map((part) => (/^\d{1,3}$/.test(part) ? Number(part) : Number.NaN));
  return octets.every((octet) => octet >= 0 && octet <= 255) ? octets : null;
}

function parseIpv6(address: string): number[] | null {
  if (!address.includes(":") || /[^0-9a-f:.]/i.test(address)) return null;
  const halves = address.split("::");
  if (halves.length > 2) return null;
  const groupsOf = (part: string) => (part === "" ? [] : part.split(":"));
  const head = groupsOf(halves[0]);
  const tail = halves.length === 2 ? groupsOf(halves[1]) : [];
  const expand = (groups: string[]) => {
    const values: number[] = [];
    for (const [index, group] of groups.entries()) {
      if (group.includes(".") && index === groups.length - 1) {
        const ipv4 = parseIpv4(group);
        if (!ipv4) return null;
        values.push((ipv4[0] << 8) | ipv4[1], (ipv4[2] << 8) | ipv4[3]);
      } else if (/^[0-9a-f]{1,4}$/i.test(group)) {
        values.push(Number.parseInt(group, 16));
      } else {
        return null;
      }
    }
    return values;
  };
  const headValues = expand(head);
  const tailValues = expand(tail);
  if (!headValues || !tailValues) return null;
  const missing = 8 - headValues.length - tailValues.length;
  if (halves.length === 1 ? missing !== 0 : missing < 1) return null;
  return [...headValues, ...Array<number>(missing).fill(0), ...tailValues];
}
