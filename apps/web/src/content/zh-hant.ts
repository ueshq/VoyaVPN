import type { SiteCopy } from "./types";

export const zhHant: SiteCopy = {
  languageName: "繁體中文",
  nav: {
    home: "首頁",
    support: "支援",
    privacy: "隱私",
    skip: "跳至內容",
    languages: "語言",
  },
  footer: {
    tagline: "連接你自己選擇的代理伺服器。",
    singBox: "VoyaVPN 使用 sing-box，其授權為 GPL-3.0-or-later。",
    contact: "聯絡我們",
  },
  home: {
    title: "VoyaVPN：連接你自己的代理伺服器，不收集任何資料",
    description: "VoyaVPN 透過訂閱、分享連結或 QR 碼連接你設定的代理伺服器。無需帳號，沒有統計分析，不收集任何資料。",
    headline: "為你已有的伺服器而做的 VPN 用戶端",
    sub: "匯入訂閱、分享連結或 QR 碼，選好節點即可連線。VoyaVPN 不收集任何資料。",
    download: "下載",
    features: "查看功能",
    screenshotAlt: "桌面版 VoyaVPN 已透過東京節點連線，下方顯示出口 IP 與連線時間。",
    privacy: {
      headline: "從設計上就不收集資料",
      body: "不必註冊，也沒有統計分析、當機回報或廣告程式碼。流量只送往你新增的伺服器，應用程式保存的內容只留在你的裝置上。",
      keptTitle: "只保存在裝置上",
      kept: ["你的伺服器、訂閱與設定", "每台伺服器的流量統計", "保留 7 天的診斷日誌，憑證已移除"],
      neverTitle: "從不收集",
      never: ["瀏覽紀錄或連線清單", "統計分析、當機報告或廣告識別碼", "帳號、電子郵件或付款資訊"],
      link: "閱讀隱私權政策",
    },
    featuresHeadline: "需要的功能都在這裡",
    nodes: {
      title: "所有節點，一個清單",
      body: "新增訂閱、分享連結或 QR 碼。訂閱依排程自動更新，一鍵測試單一節點或全部節點的延遲。",
      alt: "節點頁面，列出同一訂閱下的四台伺服器及各自的延遲。",
    },
    items: {
      rules: {
        title: "依網站與地區分流",
        body: "本地網站直連，其餘走代理。應用程式內建預設規則集，也可以新增自己的規則。",
      },
      vpn: {
        title: "全裝置 VPN 模式",
        body: "透過系統 VPN 接管全部流量。在 Windows 與 Linux 上也可以改用系統代理。",
      },
      selfHost: {
        title: "自建節點",
        body: "把電腦變成 VLESS 或 Shadowsocks 節點，供你的其他裝置使用，並可檢查它能否從外部連線。",
      },
      activity: {
        title: "看清每一條連線",
        body: "網路活動列出即時連線、每條連線命中的規則以及經過的節點。",
      },
      exit: {
        title: "知道從哪裡出口",
        body: "連線後顯示出口 IP 與國家或地區，以及節點是否支援 IPv6。",
      },
    },
    protocolsHeadline: "支援你已經在用的協定",
    protocolsBody: "VoyaVPN 以 sing-box 為基礎。貼上以下任一格式的連結即可連線。",
    downloadHeadline: "下載",
    downloadBody: "VoyaVPN 即將上架 App Store 及其他平台。各平台上線後，下載連結會顯示在這裡。",
    comingSoon: "即將推出",
    getIt: "取得",
    platforms: {
      macos: "macOS",
      ios: "iPhone 與 iPad",
      android: "Android",
      windows: "Windows",
      linux: "Linux",
    },
  },
  support: {
    title: "支援 | VoyaVPN",
    description: "聯絡 VoyaVPN 支援，以及關於新增伺服器、連線、日誌與個人資料的常見問題解答。",
    headline: "支援",
    intro: "大多數問題出在訂閱或伺服器上。以下解答涵蓋了常見原因；如果仍無法解決，請寫信給我們。",
    contactHeadline: "聯絡支援",
    contactBody: "請在郵件中註明你的平台、應用程式版本與具體情況。附上診斷日誌會很有幫助，匯出方式請見下方解答。",
    faqHeadline: "常見問題",
    faq: [
      {
        question: "VoyaVPN 提供伺服器嗎？",
        answer: ["不提供。VoyaVPN 是用戶端，用來連接你從服務商取得或自行架設的伺服器，本身不販售也不附帶任何伺服器。"],
      },
      {
        question: "如何新增伺服器？",
        answer: [
          "開啟「節點」，選擇「新增」。貼上訂閱網址或分享連結（例如 `vless://` 或 `ss://`），或掃描 QR 碼。預覽辨識結果後匯入。",
          "訂閱會依設定的排程自動更新。如需立即更新，開啟對應群組並選擇「更新訂閱」。",
        ],
      },
      {
        question: "連線失敗，應該檢查什麼？",
        answer: [
          "先測試節點延遲。如果測試失敗，可能是伺服器無法使用或訂閱已過期：更新訂閱後再試。",
          "在 iPhone、iPad 與 Mac 上，系統會詢問一次是否允許新增 VPN 設定。如果當時拒絕了，請重新連線並允許，或在系統的 VPN 設定中開啟 VoyaVPN。",
          "如果只有部分網站無法存取，開啟「規則」查看這部分流量被送往哪裡。",
        ],
      },
      {
        question: "為什麼有些網站沒有走代理？",
        answer: [
          "在規則模式下，命中直連規則的流量（例如預設規則中的本地網站）會直接連線。在「規則」頁把流量模式切換為「全局」，即可讓所有流量都經過代理。",
        ],
      },
      {
        question: "如何傳送診斷日誌？",
        answer: [
          "桌面版：開啟「網路活動」，選擇「執行日誌」，再選擇「匯出顯示的日誌」。行動版：開啟「設定」，進入「檢視診斷」，選擇「複製去識別化診斷」。把結果附在郵件中即可。",
          "日誌寫入前會移除密碼、UUID 等憑證。在你主動匯出或分享之前，日誌不會離開你的裝置。",
        ],
      },
      {
        question: "如何刪除我的資料？",
        answer: [
          "VoyaVPN 保存的所有內容都在你的裝置上。可以在應用程式中刪除伺服器與訂閱，解除安裝則會刪除全部內容。沒有需要註銷的帳號。",
        ],
      },
    ],
  },
  privacy: {
    title: "隱私權政策 | VoyaVPN",
    description: "VoyaVPN 不收集任何個人資料。本政策列出保存在你裝置上的內容，以及應用程式自行發出的每一類請求。",
    headline: "隱私權政策",
    effective: "生效日期：{date}",
    intro:
      "VoyaVPN 是用於連接你所設定代理伺服器的用戶端，不收集任何個人資料。本政策說明應用程式在你的裝置上保存哪些內容，以及它會自行發出哪些請求。",
    sections: [
      {
        heading: "我們收集什麼",
        body: [
          "什麼都不收集。VoyaVPN 不含任何統計分析、當機回報、廣告或遙測程式碼，也沒有使用者帳號。我們不營運任何接收你的流量或個人資訊的伺服器。",
        ],
      },
      {
        heading: "保存在你裝置上的內容",
        body: [
          "為了正常運作，應用程式只在你的裝置上保存以下內容：",
          {
            list: ["你的伺服器、訂閱與設定；", "每台伺服器的流量統計；", "保留 7 天的診斷日誌，其中的憑證已移除。"],
          },
          "只有在你自己匯出或分享時，日誌才會離開你的裝置。",
        ],
      },
      {
        heading: "你的流量",
        body: [
          "VPN 通道在你的裝置上執行，只把流量送往你設定的伺服器。VoyaVPN 從不記錄或傳輸你的瀏覽流量與連線清單。",
          "DNS 查詢由應用程式中設定的 DNS 伺服器回應：預設為 Cloudflare DNS over HTTPS，經由你的伺服器存取。VoyaVPN 不記錄這些查詢。",
          "你所選代理伺服器與 DNS 伺服器的營運方，依其各自的政策處理你的流量。",
        ],
      },
      {
        heading: "應用程式自行發出的請求",
        body: [
          "以下請求僅用於實現功能，不攜帶任何識別碼或個人資料；與任何網路請求一樣，對方能看到請求來源的 IP 位址。",
          {
            list: [
              "下載你填寫的訂閱網址，以取得伺服器清單。",
              "從 `raw.githubusercontent.com` 下載分流規則檔案。",
              "連線後，經由你自己的伺服器查詢出口 IP 位址與國家或地區（`ipwho.is`、`icanhazip.com`、`ipify.org`、`ident.me`），用於顯示流量從哪裡出口以及伺服器是否支援 IPv6。",
              "透過受測伺服器請求 `www.google.com/generate_204` 來測量延遲。",
              "桌面版在自建節點開啟期間，會在節點啟動時、此後每十分鐘以及你手動檢測時執行網路檢查。每次檢查會把所選連接埠號碼傳送到 `probe.voyavpn.wangc.ai`，由它嘗試回連這些連接埠並傳回結果；同時向 `www.cloudflare.com/cdn-cgi/trace` 查詢你的公用 IP 位址。探測服務不保存任何內容，也不記錄日誌。",
            ],
          },
        ],
      },
      {
        heading: "各平台的差異",
        body: [
          {
            list: [
              "iPhone 與 iPad：沒有自建節點功能，因此應用程式從不存取 `probe.voyavpn.wangc.ai`。",
              "Android：QR 碼由 Google ML Kit 在裝置上辨識。Google 會收到該 SDK 的效能與使用指標，SDK 也可能聯絡 Google 取得更新與相容性資訊。",
              "如果你從應用程式商店安裝，商店會依其自己的隱私權政策處理下載與購買資料。",
            ],
          },
        ],
      },
      {
        heading: "分享",
        body: ["我們不販售資料，也不與任何人分享資料，因為我們沒有任何資料。"],
      },
      {
        heading: "刪除你的資料",
        body: ["你可以隨時在應用程式中刪除伺服器、訂閱與日誌。解除安裝應用程式會刪除它保存的全部內容。"],
      },
      {
        heading: "本網站",
        body: [
          "本站不使用 Cookie、統計分析或指令碼。網站由 Cloudflare 託管，Cloudflare 會處理 IP 位址等請求資料，用於提供頁面與防範濫用。",
        ],
      },
      {
        heading: "政策變更",
        body: ["如本政策有變更，我們會更新本頁面及其生效日期。"],
      },
    ],
    contactHeading: "聯絡我們",
    contactBody: "對本政策有疑問？請寄信至 {email}。",
  },
  notFound: {
    title: "找不到頁面 | VoyaVPN",
    headline: "找不到頁面",
    body: "你要找的頁面不存在或已移動。",
    back: "返回首頁",
  },
};
