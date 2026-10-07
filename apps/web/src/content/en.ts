import type { SiteCopy } from "./types";

export const en: SiteCopy = {
  languageName: "English",
  nav: {
    home: "Home",
    support: "Support",
    privacy: "Privacy",
    skip: "Skip to content",
    languages: "Language",
  },
  footer: {
    tagline: "A client for the proxy servers you choose.",
    singBox: "VoyaVPN runs sing-box, licensed under GPL-3.0-or-later.",
    contact: "Contact",
  },
  home: {
    title: "VoyaVPN: a private client for your own proxy servers",
    description:
      "VoyaVPN connects to the proxy servers you configure, by subscription, share link or QR code. No accounts, no analytics, no data collected.",
    headline: "A VPN client for the servers you already have",
    sub: "Import a subscription, share link or QR code, pick a node and connect. VoyaVPN collects no data.",
    download: "Download",
    features: "See features",
    screenshotAlt:
      "VoyaVPN on the desktop, connected through a node in Tokyo, with the exit IP and connection time below.",
    privacy: {
      headline: "Built to collect nothing",
      body: "There is no sign-up and no analytics, crash-reporting or advertising code. Your traffic goes only to the servers you add, and what the app keeps stays on your device.",
      keptTitle: "Kept on your device",
      kept: [
        "Your servers, subscriptions and settings",
        "Traffic counters for each server",
        "A 7-day diagnostic log, with credentials removed",
      ],
      neverTitle: "Never collected",
      never: [
        "Browsing history or connection lists",
        "Analytics, crash reports or advertising IDs",
        "Accounts, email addresses or payment details",
      ],
      link: "Read the privacy policy",
    },
    featuresHeadline: "Everything in one place",
    nodes: {
      title: "All your nodes in one list",
      body: "Add subscriptions, share links or QR codes. Subscriptions update on a schedule, and one click tests the latency of a single node or all of them.",
      alt: "The Nodes page listing four servers from one subscription, each with its latency.",
    },
    items: {
      rules: {
        title: "Rules by site and region",
        body: "Send local sites direct and the rest through the proxy. Default rule sets ship with the app, and you can add your own.",
      },
      vpn: {
        title: "Whole-device VPN mode",
        body: "Route all traffic through a system VPN. On Windows and Linux you can use the system proxy instead.",
      },
      selfHost: {
        title: "Host your own node",
        body: "Turn a computer into a VLESS or Shadowsocks node for your other devices, with a check that it can be reached.",
      },
      activity: {
        title: "See every connection",
        body: "Network activity lists live connections, the rule each one matched and the node it went through.",
      },
      exit: {
        title: "Know where you exit",
        body: "After connecting, the app shows your exit IP and country, and whether the node supports IPv6.",
      },
    },
    protocolsHeadline: "Works with the protocols you already use",
    protocolsBody: "VoyaVPN is built on sing-box. Paste a link in any of these formats and it is ready to connect.",
    downloadHeadline: "Download",
    downloadBody:
      "VoyaVPN is coming to the App Store and to the other platforms. Links appear here as each one goes live.",
    comingSoon: "Coming soon",
    getIt: "Get it",
    platforms: {
      macos: "macOS",
      ios: "iPhone and iPad",
      android: "Android",
      windows: "Windows",
      linux: "Linux",
    },
  },
  support: {
    title: "Support | VoyaVPN",
    description:
      "Contact VoyaVPN support, and answers to common questions about adding servers, connecting, logs and your data.",
    headline: "Support",
    intro:
      "Most problems come from a subscription or a server. The answers below cover the usual causes. If they don't help, write to us.",
    contactHeadline: "Contact support",
    contactBody:
      "Email us with your platform, the app version and what happened. A diagnostic log helps; the answers below explain how to export one.",
    faqHeadline: "Common questions",
    faq: [
      {
        question: "Does VoyaVPN provide servers?",
        answer: [
          "No. VoyaVPN is a client. It connects to servers you get from a provider or run yourself, and it does not sell or include any.",
        ],
      },
      {
        question: "How do I add servers?",
        answer: [
          "Open Nodes and choose Add. Paste a subscription URL or a share link (such as `vless://` or `ss://`), or scan a QR code. Preview what was found, then import it.",
          "Subscriptions update on their own schedule. To update one now, open its group and choose Update subscription.",
        ],
      },
      {
        question: "The connection fails. What should I check?",
        answer: [
          "Test the node's latency first. If the test fails, the server may be down or the subscription out of date: update the subscription and try again.",
          "On iPhone, iPad and Mac the system asks once for permission to add a VPN configuration. If you declined, connect again and allow it, or turn VoyaVPN on in the system VPN settings.",
          "If only some sites fail, open Rules to see where that traffic is sent.",
        ],
      },
      {
        question: "Why do some sites skip the proxy?",
        answer: [
          "In Rule mode, traffic that matches a direct rule, such as local sites in the default rules, connects directly. Switch the traffic mode to Global on the Rules page to send everything through the proxy.",
        ],
      },
      {
        question: "How do I send a diagnostic log?",
        answer: [
          "On the desktop, open Network activity, choose Runtime logs, then Export shown logs. On mobile, open Settings, then View diagnostics, and choose Copy redacted diagnostics. Attach the result to your email.",
          "Passwords, UUIDs and other credentials are removed before the log is written. It stays on your device until you export or share it.",
        ],
      },
      {
        question: "How do I delete my data?",
        answer: [
          "Everything VoyaVPN stores is on your device. Delete servers and subscriptions in the app, or uninstall it to remove everything. There is no account to close.",
        ],
      },
    ],
  },
  privacy: {
    title: "Privacy Policy | VoyaVPN",
    description:
      "VoyaVPN collects no personal data. This policy lists what stays on your device and every request the app makes on its own.",
    headline: "Privacy Policy",
    effective: "Effective: {date}",
    intro:
      "VoyaVPN is a client for proxy servers that you configure. It does not collect personal data. This policy explains what the app keeps on your device and which requests it makes on its own.",
    sections: [
      {
        heading: "What we collect",
        body: [
          "Nothing. VoyaVPN contains no analytics, crash-reporting, advertising or telemetry code, and it has no user accounts. We do not run any server that receives your traffic or information about you.",
        ],
      },
      {
        heading: "What stays on your device",
        body: [
          "To work, the app stores the following on your device only:",
          {
            list: [
              "your servers, subscriptions and settings;",
              "traffic counters for each server;",
              "a diagnostic log, kept for 7 days, with credentials removed.",
            ],
          },
          "The log leaves your device only when you export or share it yourself.",
        ],
      },
      {
        heading: "Your traffic",
        body: [
          "The VPN tunnel runs on your device and sends traffic only to the servers you configured. VoyaVPN never records or transmits your browsing traffic or connection lists.",
          "DNS queries are answered by the DNS servers set in the app: Cloudflare DNS over HTTPS by default, reached through your server. VoyaVPN does not log them.",
          "The operators of the proxy and DNS servers you choose handle your traffic under their own policies.",
        ],
      },
      {
        heading: "Requests the app makes on its own",
        body: [
          "These requests are functional. They carry no identifier or personal data; like any request, they reveal the IP address they come from.",
          {
            list: [
              "It downloads the subscription URLs you enter, to get your servers.",
              "It downloads routing rule files from `raw.githubusercontent.com`.",
              "After connecting, it looks up your exit IP address and country through your own server (`ipwho.is`, `icanhazip.com`, `ipify.org`, `ident.me`), to show where traffic exits and whether the server supports IPv6.",
              "It measures latency with a request to `www.google.com/generate_204` through the server being tested.",
              "On desktop, when you run the self-hosted node's network check, it sends the chosen port numbers to `probe.voyavpn.app`, which tries to connect back to those ports and returns the result, and it asks `www.cloudflare.com/cdn-cgi/trace` for your public IP address. The probe service stores nothing and keeps no logs.",
            ],
          },
        ],
      },
      {
        heading: "Platform differences",
        body: [
          {
            list: [
              "iPhone and iPad: there is no self-hosted node, so the app never calls `probe.voyavpn.app`.",
              "Android: QR codes are decoded on the device by Google ML Kit. Google receives the SDK's performance and usage metrics, and the SDK may contact Google for updates and compatibility information.",
              "If you install from an app store, the store handles download and purchase data under its own privacy policy.",
            ],
          },
        ],
      },
      {
        heading: "Sharing",
        body: ["We do not sell or share data with anyone. We have none to share."],
      },
      {
        heading: "Deleting your data",
        body: [
          "You can delete servers, subscriptions and logs in the app at any time. Uninstalling the app removes everything it stored.",
        ],
      },
      {
        heading: "This website",
        body: [
          "This site uses no cookies, analytics or scripts. It is hosted by Cloudflare, which processes request data such as IP addresses to deliver the pages and protect them from abuse.",
        ],
      },
      {
        heading: "Changes",
        body: ["If this policy changes, we will update this page and its effective date."],
      },
    ],
    contactHeading: "Contact",
    contactBody: "Questions about this policy? Email us at {email}.",
  },
  notFound: {
    title: "Page not found | VoyaVPN",
    headline: "Page not found",
    body: "The page you asked for does not exist or has moved.",
    back: "Go to the home page",
  },
};
