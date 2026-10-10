import type { SiteCopy } from "./types";

export const zhHans: SiteCopy = {
  languageName: "简体中文",
  nav: {
    home: "首页",
    support: "支持",
    privacy: "隐私",
    skip: "跳到正文",
    languages: "语言",
  },
  footer: {
    tagline: "连接你自己选择的代理服务器。",
    singBox: "VoyaVPN 使用 sing-box，其许可证为 GPL-3.0-or-later。",
    contact: "联系我们",
  },
  home: {
    title: "VoyaVPN：连接你自己的代理服务器，不收集任何数据",
    description: "VoyaVPN 通过订阅、分享链接或二维码连接你配置的代理服务器。无需账号，没有统计分析，不收集任何数据。",
    headline: "为你已有的服务器而做的 VPN 客户端",
    sub: "导入订阅、分享链接或二维码，选好节点即可连接。VoyaVPN 不收集任何数据。",
    download: "下载",
    features: "查看功能",
    screenshotAlt: "桌面版 VoyaVPN 已通过东京节点连接，下方显示出口 IP 和连接时长。",
    privacy: {
      headline: "从设计上就不收集数据",
      body: "不用注册，也没有统计分析、崩溃上报或广告代码。流量只发往你添加的服务器，应用保存的内容只留在你的设备上。",
      keptTitle: "只保存在设备上",
      kept: ["你的服务器、订阅和设置", "每台服务器的流量统计", "保留 7 天的诊断日志，凭据已去除"],
      neverTitle: "从不收集",
      never: ["浏览记录或连接列表", "统计分析、崩溃报告或广告标识符", "账号、邮箱或支付信息"],
      link: "阅读隐私政策",
    },
    featuresHeadline: "需要的功能都在这里",
    nodes: {
      title: "所有节点，一个列表",
      body: "添加订阅、分享链接或二维码。订阅按计划自动更新，一键测试单个节点或全部节点的延迟。",
      alt: "节点页面，列出同一订阅下的四台服务器及各自的延迟。",
    },
    items: {
      rules: {
        title: "按网站和地区分流",
        body: "国内网站直连，其余走代理。应用内置默认规则集，也可以添加自己的规则。",
      },
      vpn: {
        title: "全设备 VPN 模式",
        body: "通过系统 VPN 接管全部流量。在 Windows 和 Linux 上也可以改用系统代理。",
      },
      selfHost: {
        title: "自建节点",
        body: "把电脑变成 VLESS 或 Shadowsocks 节点，供你的其他设备使用，并可检查它能否从外部访问。",
      },
      activity: {
        title: "看清每一条连接",
        body: "网络活动列出实时连接、每条连接命中的规则以及经过的节点。",
      },
      exit: {
        title: "知道从哪里出口",
        body: "连接后显示出口 IP 和国家或地区，以及节点是否支持 IPv6。",
      },
    },
    protocolsHeadline: "支持你已经在用的协议",
    protocolsBody: "VoyaVPN 基于 sing-box。粘贴以下任意格式的链接即可连接。",
    downloadHeadline: "下载",
    downloadBody: "VoyaVPN 即将上架 App Store 及其他平台。各平台上线后，下载链接会显示在这里。",
    comingSoon: "即将推出",
    getIt: "获取",
    platforms: {
      macos: "macOS",
      ios: "iPhone 和 iPad",
      android: "Android",
      windows: "Windows",
      linux: "Linux",
    },
  },
  support: {
    title: "支持 | VoyaVPN",
    description: "联系 VoyaVPN 支持，以及关于添加服务器、连接、日志和个人数据的常见问题解答。",
    headline: "支持",
    intro: "大多数问题出在订阅或服务器上。下面的解答涵盖了常见原因；如果仍无法解决，请给我们写信。",
    contactHeadline: "联系支持",
    contactBody: "请在邮件中注明你的平台、应用版本和具体情况。附上诊断日志会很有帮助，导出方法见下方解答。",
    faqHeadline: "常见问题",
    faq: [
      {
        question: "VoyaVPN 提供服务器吗？",
        answer: ["不提供。VoyaVPN 是客户端，用来连接你从服务商获得或自己搭建的服务器，本身不出售也不附带任何服务器。"],
      },
      {
        question: "如何添加服务器？",
        answer: [
          "打开“节点”，选择“添加”。粘贴订阅 URL 或分享链接（例如 `vless://` 或 `ss://`），或扫描二维码。预览识别结果后导入。",
          "订阅会按设定的计划自动更新。如需立即更新，打开对应分组并选择“更新订阅”。",
        ],
      },
      {
        question: "连接失败，应该检查什么？",
        answer: [
          "先测试节点延迟。如果测试失败，可能是服务器不可用或订阅已过期：更新订阅后再试。",
          "在 iPhone、iPad 和 Mac 上，系统会请求一次添加 VPN 配置的权限。如果当时拒绝了，请重新连接并允许，或在系统的 VPN 设置中开启 VoyaVPN。",
          "如果只有部分网站无法访问，打开“规则”查看这部分流量被发往哪里。",
        ],
      },
      {
        question: "为什么有些网站没有走代理？",
        answer: [
          "在规则模式下，命中直连规则的流量（例如默认规则中的国内网站）会直接连接。在“规则”页把流量模式切换为“全局”，即可让所有流量都经过代理。",
        ],
      },
      {
        question: "如何发送诊断日志？",
        answer: [
          "桌面版：打开“网络活动”，选择“运行日志”，再选择“导出显示的日志”。移动版：打开“设置”，进入“查看诊断”，选择“复制脱敏诊断”。把结果附在邮件中即可。",
          "日志写入前会去除密码、UUID 等凭据。在你主动导出或分享之前，日志不会离开你的设备。",
        ],
      },
      {
        question: "如何删除我的数据？",
        answer: [
          "VoyaVPN 保存的所有内容都在你的设备上。可以在应用中删除服务器和订阅，卸载应用则会删除全部内容。没有需要注销的账号。",
        ],
      },
    ],
  },
  privacy: {
    title: "隐私政策 | VoyaVPN",
    description: "VoyaVPN 不收集任何个人数据。本政策列出保存在你设备上的内容，以及应用自行发出的每一类请求。",
    headline: "隐私政策",
    effective: "生效日期：{date}",
    intro:
      "VoyaVPN 是用于连接你所配置代理服务器的客户端，不收集任何个人数据。本政策说明应用在你的设备上保存哪些内容，以及它会自行发出哪些请求。",
    sections: [
      {
        heading: "我们收集什么",
        body: [
          "什么都不收集。VoyaVPN 不含任何统计分析、崩溃上报、广告或遥测代码，也没有用户账号。我们不运营任何接收你的流量或个人信息的服务器。",
        ],
      },
      {
        heading: "保存在你设备上的内容",
        body: [
          "为了正常工作，应用只在你的设备上保存以下内容：",
          {
            list: ["你的服务器、订阅和设置；", "每台服务器的流量统计；", "保留 7 天的诊断日志，其中的凭据已去除。"],
          },
          "只有你自己导出或分享时，日志才会离开你的设备。",
        ],
      },
      {
        heading: "你的流量",
        body: [
          "VPN 隧道在你的设备上运行，只把流量发往你配置的服务器。VoyaVPN 从不记录或传输你的浏览流量和连接列表。",
          "DNS 查询由应用中设置的 DNS 服务器解答：默认为 Cloudflare DNS over HTTPS，经由你的服务器访问。VoyaVPN 不记录这些查询。",
          "你所选代理服务器和 DNS 服务器的运营方，按照其各自的政策处理你的流量。",
        ],
      },
      {
        heading: "应用自行发出的请求",
        body: [
          "以下请求仅用于实现功能，不携带任何标识符或个人数据；与任何网络请求一样，对方能看到请求来源的 IP 地址。",
          {
            list: [
              "下载你填写的订阅 URL，以获取服务器列表。",
              "从 `raw.githubusercontent.com` 下载分流规则文件。",
              "连接后，经由你自己的服务器查询出口 IP 地址和国家或地区（`ipwho.is`、`icanhazip.com`、`ipify.org`、`ident.me`），用于显示流量从哪里出口以及服务器是否支持 IPv6。",
              "通过被测服务器请求 `www.google.com/generate_204` 来测量延迟。",
              "桌面版在自建节点开启期间，会在节点启动时、此后每十分钟以及你手动检测时运行网络检查。每次检查会把所选端口号发送到 `probe.voyavpn.wangc.ai`，由它尝试回连这些端口并返回结果；同时向 `www.cloudflare.com/cdn-cgi/trace` 查询你的公网 IP 地址。探测服务不保存任何内容，也不记录日志。",
            ],
          },
        ],
      },
      {
        heading: "各平台的差异",
        body: [
          {
            list: [
              "iPhone 和 iPad：没有自建节点功能，因此应用从不访问 `probe.voyavpn.wangc.ai`。",
              "Android：二维码由 Google ML Kit 在设备上识别。Google 会收到该 SDK 的性能和使用指标，SDK 也可能联系 Google 获取更新和兼容性信息。",
              "如果你从应用商店安装，商店会按照其自己的隐私政策处理下载和购买数据。",
            ],
          },
        ],
      },
      {
        heading: "共享",
        body: ["我们不出售数据，也不与任何人共享数据，因为我们没有任何数据。"],
      },
      {
        heading: "删除你的数据",
        body: ["你可以随时在应用中删除服务器、订阅和日志。卸载应用会删除它保存的全部内容。"],
      },
      {
        heading: "本网站",
        body: [
          "本站不使用 Cookie、统计分析或脚本。网站由 Cloudflare 托管，Cloudflare 会处理 IP 地址等请求数据，用于提供页面和防范滥用。",
        ],
      },
      {
        heading: "政策变更",
        body: ["如本政策有变更，我们会更新本页面及其生效日期。"],
      },
    ],
    contactHeading: "联系我们",
    contactBody: "对本政策有疑问？请发邮件至 {email}。",
  },
  notFound: {
    title: "页面不存在 | VoyaVPN",
    headline: "页面不存在",
    body: "你访问的页面不存在或已移动。",
    back: "返回首页",
  },
};
