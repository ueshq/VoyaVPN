import type { ProfileListEntry } from "../../src/ipc/bindings";

export const savedNodeFixture: ProfileListEntry = {
  isActive: true,
  profile: {
    id: "profile-0",
    remarks: "Server 0",
    displayLog: true,
    subscriptionId: null,
    tls: null,
    transport: null,
    protocol: { kind: "vmess", server: { address: "node-0.example.test", port: 443 }, cipher: "auto", uuid: "uuid-0" },
  },
  metrics: { delayMs: 40, ipInfo: null, outcome: null, sort: 0 },
  traffic: { date: 1, todayUpload: 0, todayDownload: 0, totalUpload: 0, totalDownload: 0 },
};
