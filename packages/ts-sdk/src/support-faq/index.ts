export const FAQ_SCHEMA_VERSION = 1 as const;
export const FAQ_DEFAULT_LOCALE = "zh-Hant" as const;
export const FAQ_SUPPORTED_LOCALES = [
  "zh-Hant",
  "zh-Hans",
  "en",
  "id",
  "vi",
  "tl",
] as const;

export type FaqSchemaVersion = typeof FAQ_SCHEMA_VERSION;
export type FaqLocale = (typeof FAQ_SUPPORTED_LOCALES)[number];
export type FaqLocalizedText = Partial<Record<FaqLocale, string>>;

export interface FaqCatalog {
  schemaVersion: FaqSchemaVersion;
  updatedAt: string;
  updatedBy: string;
  sections: FaqSection[];
}

export interface FaqSection {
  id: string;
  order: number;
  enabled: boolean;
  title: FaqLocalizedText;
  items: FaqItem[];
}

export interface FaqItem {
  id: string;
  order: number;
  enabled: boolean;
  question: FaqLocalizedText;
  answer: FaqLocalizedText;
}

export interface PublicFaqSection {
  id: string;
  title: string;
  items: PublicFaqItem[];
}

export interface PublicFaqItem {
  id: string;
  question: string;
  answer: string;
}

export interface CreateDefaultFaqCatalogOptions {
  updatedAt?: string;
  updatedBy?: string;
}

export interface FaqCatalogStore {
  load(): Promise<FaqCatalog | null>;
  save(catalog: FaqCatalog): Promise<void>;
  reset(): Promise<void>;
}

export interface FaqKvNamespace {
  get(key: string): Promise<string | null>;
  put(key: string, value: string): Promise<void>;
  delete(key: string): Promise<void>;
}

export interface KvFaqCatalogStoreOptions {
  key?: string;
}

export class FaqCatalogValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "FaqCatalogValidationError";
  }
}

const idPattern = /^[A-Za-z0-9][A-Za-z0-9_-]{1,63}$/;
const maxSections = 20;
const maxItemsPerSection = 40;
const maxTextLength = 2_000;
export const DEFAULT_FAQ_KV_KEY = "support:faq:catalog:v1";

const defaultSections: Array<
  Omit<FaqSection, "items"> & {
    items: FaqItem[];
  }
> = [
  {
    id: "account",
    order: 10,
    enabled: true,
    title: { "zh-Hant": "帳號與登入", en: "Account and sign-in" },
    items: [
      item(
        "forgotPassword",
        10,
        "忘記密碼怎麼辦？",
        "請在登入頁使用忘記密碼流程。系統會寄送重設連結到已驗證的電子郵件信箱。",
        "How do I reset my password?",
        "Use the forgot-password flow on the sign-in page. We will send a reset link to your verified email address.",
      ),
      item(
        "verificationCode",
        20,
        "收不到驗證碼怎麼辦？",
        "請先確認垃圾郵件匣與手機訊號。若仍無法收到，請透過客服入口建立工單。",
        "What if I cannot receive a verification code?",
        "Check spam folders and mobile reception first. If it still fails, open a ticket from the support entry point.",
      ),
      item(
        "frozen",
        30,
        "帳號被凍結時可以做什麼？",
        "凍結通常與安全、KYC 或風控事件有關。請查看通知信並依客服指示提供資料。",
        "What can I do if my account is frozen?",
        "Freezes usually relate to security, KYC, or risk events. Review the notice email and provide the requested support evidence.",
      ),
      item(
        "multiAccount",
        40,
        "可以建立多個帳號嗎？",
        "不可以。每位玩家只能持有一個帳號，多帳號或共用帳號可能導致限制或關閉。",
        "Can I create multiple accounts?",
        "No. Each player may hold one account only. Multi-accounting or shared accounts may lead to restrictions or closure.",
      ),
    ],
  },
  {
    id: "topup",
    order: 20,
    enabled: true,
    title: { "zh-Hant": "加值與點數", en: "Top-up and points" },
    items: [
      item(
        "missingPoints",
        10,
        "加值後點數沒有入帳怎麼辦？",
        "多數付款會在五分鐘內入帳。超過三十分鐘仍未入帳時，請附上交易時間與付款證明聯絡客服。",
        "What if points do not arrive after top-up?",
        "Most payments post within five minutes. If points are still missing after thirty minutes, contact support with payment time and proof.",
      ),
      item(
        "cashout",
        20,
        "點數可以提領嗎？",
        "提領流程依所在地區、KYC 等級與支付通道而定。請以帳戶頁顯示的可用功能為準。",
        "Can points be cashed out?",
        "Cash-out availability depends on jurisdiction, KYC tier, and payment rails. Follow the features shown in your account.",
      ),
      item(
        "monthlyLimit",
        30,
        "每月加值上限是多少？",
        "每月上限依責任博弈設定與 KYC 等級決定。較高額度可能需要完成額外審查。",
        "What is my monthly top-up limit?",
        "Monthly limits depend on responsible-gaming settings and KYC tier. Higher limits may require additional review.",
      ),
      item(
        "raiseLimit",
        40,
        "如何申請提高上限？",
        "請先完成必要 KYC，再於責任博弈設定中提出調整。提高上限不會即時生效。",
        "How do I request a higher limit?",
        "Complete the required KYC first, then request the change from responsible-gaming settings. Higher limits do not take effect immediately.",
      ),
    ],
  },
  {
    id: "kyc",
    order: 30,
    enabled: true,
    title: { "zh-Hant": "身分驗證 KYC", en: "KYC verification" },
    items: [
      item(
        "whyKyc",
        10,
        "為什麼需要 KYC？",
        "KYC 用於確認年齡、身分與合規限制，並支援支付、提領與風控流程。",
        "Why is KYC required?",
        "KYC confirms age, identity, and compliance restrictions, and supports payment, cash-out, and risk workflows.",
      ),
      item(
        "failed",
        20,
        "KYC 失敗後怎麼辦？",
        "請依畫面或客服通知補件。若你認為判定有誤，可以透過爭議流程要求覆核。",
        "What happens after a failed KYC review?",
        "Follow the on-screen or support request for additional documents. If you believe the decision is wrong, request review through disputes.",
      ),
      item(
        "documentImages",
        30,
        "證件影像如何保存？",
        "證件資料以加密方式保存並受存取控管。一般客服無法直接查看證件影像。",
        "How are document images stored?",
        "Documents are encrypted and access-controlled. General support staff cannot directly view document images.",
      ),
    ],
  },
  {
    id: "settlement",
    order: 40,
    enabled: true,
    title: { "zh-Hant": "結算與爭議", en: "Settlement and disputes" },
    items: [
      item(
        "oracle",
        10,
        "結算依據是什麼？",
        "每個市場會列出 oracle source。結算只能依預先揭露的來源與規則執行。",
        "What determines settlement?",
        "Each market lists its oracle source. Settlement follows the pre-disclosed source and rules.",
      ),
      item(
        "dispute",
        20,
        "對結算結果有疑問怎麼辦？",
        "請在爭議期限內提出申訴並附上證據。逾期案件可能無法重啟。",
        "What if I disagree with settlement?",
        "Submit a dispute within the dispute window and include evidence. Late cases may not be reopened.",
      ),
      item(
        "delist",
        30,
        "市場下架後會怎麼處理？",
        "下架市場會依規則進入取消、延後或人工審查流程，並保留可稽核紀錄。",
        "What happens when a market is delisted?",
        "A delisted market follows cancellation, extension, or manual-review rules, with auditable records retained.",
      ),
    ],
  },
  {
    id: "rg",
    order: 50,
    enabled: true,
    title: { "zh-Hant": "責任博弈", en: "Responsible gaming" },
    items: [
      item(
        "tooMuch",
        10,
        "覺得自己玩太多怎麼辦？",
        "請優先使用責任博弈工具設定限制、冷靜期或自我排除。必要時可聯絡專責客服。",
        "What if I feel I am playing too much?",
        "Use responsible-gaming tools first to set limits, cooldowns, or self-exclusion. Contact dedicated support when needed.",
      ),
      item(
        "selfExclusion",
        20,
        "自我排除可以解除嗎？",
        "自我排除一旦生效即具約束力，期限內不會因臨時要求而提前解除。",
        "Can self-exclusion be lifted?",
        "Once active, self-exclusion is binding and will not be lifted early on request during the selected period.",
      ),
      item(
        "thirdParty",
        30,
        "家人或第三方可以求助嗎？",
        "可以。客服會記錄線索並依責任博弈流程評估，但不會向第三方揭露帳戶細節。",
        "Can family members or third parties ask for help?",
        "Yes. Support will record the concern and assess it under responsible-gaming procedures without disclosing account details.",
      ),
    ],
  },
  {
    id: "security",
    order: 60,
    enabled: true,
    title: { "zh-Hant": "安全", en: "Security" },
    items: [
      item(
        "stolen",
        10,
        "懷疑帳號被盜怎麼辦？",
        "請立即聯絡客服並變更密碼。平台可能暫時限制帳戶以保護資產與資料。",
        "What if I suspect account takeover?",
        "Contact support immediately and change your password. The platform may temporarily restrict the account to protect assets and data.",
      ),
      item(
        "mfa",
        20,
        "支援多因素驗證嗎？",
        "MFA 將逐步提供。啟用後，提款、敏感設定與高風險登入會要求額外驗證。",
        "Is multi-factor authentication supported?",
        "MFA will be rolled out progressively. Once enabled, withdrawals, sensitive settings, and high-risk sign-ins require extra verification.",
      ),
    ],
  },
];

const localizedSectionTitles: Record<string, FaqLocalizedText> = {
  account: {
    "zh-Hans": "账号与登录",
    id: "Akun dan masuk",
    vi: "Tài khoản và đăng nhập",
    tl: "Account at pag-sign in",
  },
  topup: {
    "zh-Hans": "充值与点数",
    id: "Top-up dan poin",
    vi: "Nạp tiền và điểm",
    tl: "Top-up at points",
  },
  kyc: {
    "zh-Hans": "身份验证 KYC",
    id: "Verifikasi identitas KYC",
    vi: "Xác minh danh tính KYC",
    tl: "Pag-verify ng pagkakakilanlan KYC",
  },
  settlement: {
    "zh-Hans": "结算与争议",
    id: "Penyelesaian dan sengketa",
    vi: "Thanh toán và khiếu nại",
    tl: "Settlement at disputes",
  },
  rg: {
    "zh-Hans": "责任博彩",
    id: "Bermain secara bertanggung jawab",
    vi: "Chơi có trách nhiệm",
    tl: "Responsableng paglalaro",
  },
  security: {
    "zh-Hans": "安全",
    id: "Keamanan",
    vi: "Bảo mật",
    tl: "Seguridad",
  },
};

const localizedItemCopy: Record<
  string,
  Partial<Record<FaqLocale, { question: string; answer: string }>>
> = {
  "account.forgotPassword": {
    "zh-Hans": {
      question: "忘记密码怎么办？",
      answer:
        "请在登录页使用忘记密码流程。系统会发送重设链接到已验证的电子邮箱。",
    },
    id: {
      question: "Apa yang harus saya lakukan jika lupa kata sandi?",
      answer:
        "Gunakan alur lupa kata sandi di halaman masuk. Kami akan mengirim tautan reset ke alamat email yang sudah diverifikasi.",
    },
    vi: {
      question: "Tôi nên làm gì nếu quên mật khẩu?",
      answer:
        "Dùng quy trình quên mật khẩu trên trang đăng nhập. Chúng tôi sẽ gửi liên kết đặt lại đến địa chỉ email đã xác minh.",
    },
    tl: {
      question: "Ano ang gagawin kung nakalimutan ko ang password?",
      answer:
        "Gamitin ang forgot-password flow sa sign-in page. Magpapadala kami ng reset link sa naverify mong email address.",
    },
  },
  "account.verificationCode": {
    "zh-Hans": {
      question: "收不到验证码怎么办？",
      answer:
        "请先确认垃圾邮件夹与手机信号。若仍无法收到，请通过客服入口建立工单。",
    },
    id: {
      question: "Bagaimana jika saya tidak menerima kode verifikasi?",
      answer:
        "Periksa folder spam dan sinyal seluler terlebih dahulu. Jika masih gagal, buat tiket melalui pusat bantuan.",
    },
    vi: {
      question: "Nếu không nhận được mã xác minh thì sao?",
      answer:
        "Hãy kiểm tra thư mục spam và tín hiệu điện thoại trước. Nếu vẫn không nhận được, hãy tạo ticket qua cổng hỗ trợ.",
    },
    tl: {
      question: "Paano kung hindi ako makatanggap ng verification code?",
      answer:
        "Tingnan muna ang spam folder at mobile reception. Kung hindi pa rin gumagana, gumawa ng ticket mula sa support entry point.",
    },
  },
  "account.frozen": {
    "zh-Hans": {
      question: "账号被冻结时可以做什么？",
      answer:
        "冻结通常与安全、KYC 或风控事件有关。请查看通知邮件并按客服指示提供资料。",
    },
    id: {
      question: "Apa yang dapat saya lakukan jika akun dibekukan?",
      answer:
        "Pembekuan biasanya terkait keamanan, KYC, atau peristiwa risiko. Tinjau email pemberitahuan dan berikan bukti yang diminta support.",
    },
    vi: {
      question: "Tôi có thể làm gì khi tài khoản bị đóng băng?",
      answer:
        "Đóng băng thường liên quan đến bảo mật, KYC hoặc sự kiện rủi ro. Hãy xem email thông báo và cung cấp bằng chứng theo yêu cầu hỗ trợ.",
    },
    tl: {
      question: "Ano ang puwede kong gawin kung na-freeze ang account ko?",
      answer:
        "Karaniwang may kinalaman sa seguridad, KYC, o risk event ang freeze. Basahin ang notice email at ibigay ang ebidensiyang hinihingi ng support.",
    },
  },
  "account.multiAccount": {
    "zh-Hans": {
      question: "可以建立多个账号吗？",
      answer:
        "不可以。每位玩家只能持有一个账号，多账号或共用账号可能导致限制或关闭。",
    },
    id: {
      question: "Bolehkah saya membuat beberapa akun?",
      answer:
        "Tidak. Setiap pemain hanya boleh memiliki satu akun. Multi-akun atau akun bersama dapat menyebabkan pembatasan atau penutupan.",
    },
    vi: {
      question: "Tôi có thể tạo nhiều tài khoản không?",
      answer:
        "Không. Mỗi người chơi chỉ được giữ một tài khoản. Nhiều tài khoản hoặc tài khoản dùng chung có thể dẫn đến hạn chế hoặc đóng tài khoản.",
    },
    tl: {
      question: "Puwede ba akong gumawa ng maraming account?",
      answer:
        "Hindi. Isang account lang ang puwede sa bawat player. Maaaring magresulta sa restrictions o closure ang multi-accounting o shared accounts.",
    },
  },
  "topup.missingPoints": {
    "zh-Hans": {
      question: "充值后点数没有到账怎么办？",
      answer:
        "多数付款会在五分钟内到账。超过三十分钟仍未到账时，请附上交易时间与付款证明联系客服。",
    },
    id: {
      question: "Bagaimana jika poin belum masuk setelah top-up?",
      answer:
        "Sebagian besar pembayaran masuk dalam lima menit. Jika poin masih belum masuk setelah tiga puluh menit, hubungi support dengan waktu pembayaran dan bukti.",
    },
    vi: {
      question: "Nếu điểm chưa vào sau khi nạp tiền thì sao?",
      answer:
        "Phần lớn thanh toán sẽ được ghi nhận trong năm phút. Nếu sau ba mươi phút điểm vẫn chưa vào, hãy liên hệ hỗ trợ kèm thời gian và bằng chứng thanh toán.",
    },
    tl: {
      question: "Paano kung hindi dumating ang points pagkatapos ng top-up?",
      answer:
        "Karamihan ng payments ay napo-post sa loob ng limang minuto. Kung wala pa rin ang points pagkatapos ng tatlumpung minuto, kontakin ang support kasama ang oras at proof of payment.",
    },
  },
  "topup.cashout": {
    "zh-Hans": {
      question: "点数可以提现吗？",
      answer:
        "提现流程依所在地区、KYC 等级与支付通道而定。请以账户页显示的可用功能为准。",
    },
    id: {
      question: "Apakah poin dapat ditarik?",
      answer:
        "Ketersediaan penarikan bergantung pada yurisdiksi, tingkat KYC, dan jalur pembayaran. Ikuti fitur yang tersedia di akun Anda.",
    },
    vi: {
      question: "Điểm có thể rút ra không?",
      answer:
        "Khả năng rút tiền phụ thuộc vào khu vực pháp lý, cấp KYC và kênh thanh toán. Hãy làm theo các tính năng hiển thị trong tài khoản.",
    },
    tl: {
      question: "Puwede bang i-cash out ang points?",
      answer:
        "Nakadepende ang cash-out availability sa jurisdiction, KYC tier, at payment rails. Sundin ang mga feature na nakikita sa account mo.",
    },
  },
  "topup.monthlyLimit": {
    "zh-Hans": {
      question: "每月充值上限是多少？",
      answer:
        "每月上限依责任博彩设置与 KYC 等级决定。较高额度可能需要完成额外审查。",
    },
    id: {
      question: "Berapa batas top-up bulanan saya?",
      answer:
        "Batas bulanan bergantung pada pengaturan bermain bertanggung jawab dan tingkat KYC. Batas lebih tinggi mungkin memerlukan peninjauan tambahan.",
    },
    vi: {
      question: "Hạn mức nạp tiền hằng tháng của tôi là bao nhiêu?",
      answer:
        "Hạn mức hằng tháng phụ thuộc vào cài đặt chơi có trách nhiệm và cấp KYC. Hạn mức cao hơn có thể cần đánh giá bổ sung.",
    },
    tl: {
      question: "Ano ang monthly top-up limit ko?",
      answer:
        "Nakadepende ang monthly limits sa responsible-gaming settings at KYC tier. Maaaring kailangan ng karagdagang review para sa mas mataas na limits.",
    },
  },
  "topup.raiseLimit": {
    "zh-Hans": {
      question: "如何申请提高上限？",
      answer:
        "请先完成必要 KYC，再于责任博彩设置中提出调整。提高上限不会即时生效。",
    },
    id: {
      question: "Bagaimana cara meminta batas yang lebih tinggi?",
      answer:
        "Selesaikan KYC yang diperlukan terlebih dahulu, lalu ajukan perubahan dari pengaturan bermain bertanggung jawab. Batas yang lebih tinggi tidak langsung berlaku.",
    },
    vi: {
      question: "Làm sao để yêu cầu nâng hạn mức?",
      answer:
        "Hãy hoàn tất KYC bắt buộc trước, sau đó yêu cầu thay đổi trong cài đặt chơi có trách nhiệm. Hạn mức cao hơn sẽ không có hiệu lực ngay lập tức.",
    },
    tl: {
      question: "Paano ako hihiling ng mas mataas na limit?",
      answer:
        "Kumpletuhin muna ang kinakailangang KYC, pagkatapos ay hilingin ang pagbabago mula sa responsible-gaming settings. Hindi agad magkakabisa ang mas mataas na limit.",
    },
  },
  "kyc.whyKyc": {
    "zh-Hans": {
      question: "为什么需要 KYC？",
      answer: "KYC 用于确认年龄、身份与合规限制，并支持支付、提现与风控流程。",
    },
    id: {
      question: "Mengapa KYC diperlukan?",
      answer:
        "KYC mengonfirmasi usia, identitas, dan pembatasan kepatuhan, serta mendukung pembayaran, penarikan, dan alur risiko.",
    },
    vi: {
      question: "Tại sao cần KYC?",
      answer:
        "KYC xác nhận tuổi, danh tính và các giới hạn tuân thủ, đồng thời hỗ trợ thanh toán, rút tiền và quy trình quản lý rủi ro.",
    },
    tl: {
      question: "Bakit kailangan ang KYC?",
      answer:
        "Kinukumpirma ng KYC ang edad, identity, at compliance restrictions, at sinusuportahan nito ang payment, cash-out, at risk workflows.",
    },
  },
  "kyc.failed": {
    "zh-Hans": {
      question: "KYC 失败后怎么办？",
      answer:
        "请按页面或客服通知补件。若你认为判定有误，可以通过争议流程要求复核。",
    },
    id: {
      question: "Apa yang terjadi setelah tinjauan KYC gagal?",
      answer:
        "Ikuti instruksi di layar atau permintaan support untuk dokumen tambahan. Jika Anda yakin keputusan salah, minta peninjauan melalui proses sengketa.",
    },
    vi: {
      question: "Điều gì xảy ra sau khi KYC không đạt?",
      answer:
        "Hãy làm theo yêu cầu trên màn hình hoặc từ bộ phận hỗ trợ để bổ sung tài liệu. Nếu bạn cho rằng quyết định sai, hãy yêu cầu xem xét qua quy trình khiếu nại.",
    },
    tl: {
      question: "Ano ang mangyayari pagkatapos bumagsak ang KYC review?",
      answer:
        "Sundin ang on-screen o support request para sa karagdagang documents. Kung sa tingin mo mali ang desisyon, humiling ng review sa disputes process.",
    },
  },
  "kyc.documentImages": {
    "zh-Hans": {
      question: "证件影像如何保存？",
      answer:
        "证件资料以加密方式保存并受访问控制。一般客服无法直接查看证件影像。",
    },
    id: {
      question: "Bagaimana gambar dokumen disimpan?",
      answer:
        "Dokumen dienkripsi dan dikontrol aksesnya. Staf support umum tidak dapat melihat gambar dokumen secara langsung.",
    },
    vi: {
      question: "Ảnh chứng từ được lưu trữ như thế nào?",
      answer:
        "Tài liệu được mã hóa và kiểm soát truy cập. Nhân viên hỗ trợ thông thường không thể xem trực tiếp ảnh chứng từ.",
    },
    tl: {
      question: "Paano iniimbak ang document images?",
      answer:
        "Encrypted at access-controlled ang documents. Hindi direktang makikita ng general support staff ang document images.",
    },
  },
  "settlement.oracle": {
    "zh-Hans": {
      question: "结算依据是什么？",
      answer:
        "每个市场会列出 oracle source。结算只能依预先披露的来源与规则执行。",
    },
    id: {
      question: "Apa yang menentukan penyelesaian?",
      answer:
        "Setiap market mencantumkan oracle source. Penyelesaian mengikuti sumber dan aturan yang telah diungkapkan sebelumnya.",
    },
    vi: {
      question: "Điều gì quyết định kết quả thanh toán?",
      answer:
        "Mỗi thị trường sẽ liệt kê oracle source. Việc thanh toán chỉ thực hiện theo nguồn và quy tắc đã công bố trước.",
    },
    tl: {
      question: "Ano ang batayan ng settlement?",
      answer:
        "Nakalista sa bawat market ang oracle source. Susunod ang settlement sa naunang disclosed source at rules.",
    },
  },
  "settlement.dispute": {
    "zh-Hans": {
      question: "对结算结果有疑问怎么办？",
      answer: "请在争议期限内提出申诉并附上证据。逾期案件可能无法重启。",
    },
    id: {
      question: "Bagaimana jika saya tidak setuju dengan hasil penyelesaian?",
      answer:
        "Ajukan sengketa dalam periode sengketa dan sertakan bukti. Kasus yang terlambat mungkin tidak dapat dibuka kembali.",
    },
    vi: {
      question: "Nếu tôi không đồng ý với kết quả thanh toán thì sao?",
      answer:
        "Hãy nộp khiếu nại trong thời hạn tranh chấp và kèm bằng chứng. Các trường hợp quá hạn có thể không được mở lại.",
    },
    tl: {
      question: "Paano kung hindi ako sang-ayon sa settlement?",
      answer:
        "Mag-submit ng dispute sa loob ng dispute window at isama ang ebidensiya. Maaaring hindi na mabuksan muli ang late cases.",
    },
  },
  "settlement.delist": {
    "zh-Hans": {
      question: "市场下架后会怎么处理？",
      answer:
        "下架市场会依规则进入取消、延后或人工审查流程，并保留可稽核记录。",
    },
    id: {
      question: "Apa yang terjadi saat market dihapus?",
      answer:
        "Market yang dihapus mengikuti aturan pembatalan, perpanjangan, atau tinjauan manual, dengan catatan audit tetap disimpan.",
    },
    vi: {
      question: "Điều gì xảy ra khi thị trường bị gỡ bỏ?",
      answer:
        "Thị trường bị gỡ bỏ sẽ theo quy tắc hủy, gia hạn hoặc đánh giá thủ công, và lưu lại hồ sơ có thể kiểm toán.",
    },
    tl: {
      question: "Ano ang mangyayari kapag na-delist ang market?",
      answer:
        "Susunod ang delisted market sa cancellation, extension, o manual-review rules, at pananatilihin ang auditable records.",
    },
  },
  "rg.tooMuch": {
    "zh-Hans": {
      question: "觉得自己玩太多怎么办？",
      answer:
        "请优先使用责任博彩工具设置限制、冷静期或自我排除。必要时可联系专责客服。",
    },
    id: {
      question: "Bagaimana jika saya merasa bermain terlalu banyak?",
      answer:
        "Gunakan alat bermain bertanggung jawab terlebih dahulu untuk mengatur batas, cooldown, atau self-exclusion. Hubungi support khusus bila perlu.",
    },
    vi: {
      question: "Nếu tôi cảm thấy mình chơi quá nhiều thì sao?",
      answer:
        "Hãy ưu tiên dùng công cụ chơi có trách nhiệm để đặt giới hạn, thời gian tạm nghỉ hoặc tự loại trừ. Liên hệ hỗ trợ chuyên trách khi cần.",
    },
    tl: {
      question: "Paano kung pakiramdam ko sobra na ang paglalaro ko?",
      answer:
        "Gamitin muna ang responsible-gaming tools para magtakda ng limits, cooldowns, o self-exclusion. Kontakin ang dedicated support kung kailangan.",
    },
  },
  "rg.selfExclusion": {
    "zh-Hans": {
      question: "自我排除可以解除吗？",
      answer: "自我排除一旦生效即具约束力，期限内不会因临时要求而提前解除。",
    },
    id: {
      question: "Apakah self-exclusion dapat dicabut?",
      answer:
        "Setelah aktif, self-exclusion bersifat mengikat dan tidak akan dicabut lebih awal atas permintaan selama periode yang dipilih.",
    },
    vi: {
      question: "Có thể gỡ tự loại trừ không?",
      answer:
        "Khi đã kích hoạt, tự loại trừ có tính ràng buộc và sẽ không được gỡ sớm theo yêu cầu trong thời hạn đã chọn.",
    },
    tl: {
      question: "Puwede bang alisin ang self-exclusion?",
      answer:
        "Kapag active na, binding ang self-exclusion at hindi ito aalisin nang maaga kapag hiniling sa loob ng napiling period.",
    },
  },
  "rg.thirdParty": {
    "zh-Hans": {
      question: "家人或第三方可以求助吗？",
      answer:
        "可以。客服会记录线索并依责任博彩流程评估，但不会向第三方披露账户细节。",
    },
    id: {
      question: "Bisakah keluarga atau pihak ketiga meminta bantuan?",
      answer:
        "Ya. Support akan mencatat kekhawatiran dan menilainya sesuai prosedur bermain bertanggung jawab tanpa mengungkap detail akun.",
    },
    vi: {
      question: "Người thân hoặc bên thứ ba có thể yêu cầu trợ giúp không?",
      answer:
        "Có. Bộ phận hỗ trợ sẽ ghi nhận lo ngại và đánh giá theo quy trình chơi có trách nhiệm, nhưng không tiết lộ chi tiết tài khoản.",
    },
    tl: {
      question: "Puwede bang humingi ng tulong ang pamilya o third parties?",
      answer:
        "Oo. Ire-record ng support ang concern at susuriin ito sa ilalim ng responsible-gaming procedures nang hindi naglalabas ng account details.",
    },
  },
  "security.stolen": {
    "zh-Hans": {
      question: "怀疑账号被盗怎么办？",
      answer:
        "请立即联系客服并更改密码。平台可能暂时限制账户以保护资产与资料。",
    },
    id: {
      question:
        "Apa yang harus saya lakukan jika mencurigai pengambilalihan akun?",
      answer:
        "Segera hubungi support dan ubah kata sandi. Platform dapat membatasi akun sementara untuk melindungi aset dan data.",
    },
    vi: {
      question: "Tôi nên làm gì nếu nghi tài khoản bị chiếm đoạt?",
      answer:
        "Hãy liên hệ hỗ trợ ngay và đổi mật khẩu. Nền tảng có thể tạm thời hạn chế tài khoản để bảo vệ tài sản và dữ liệu.",
    },
    tl: {
      question:
        "Ano ang gagawin kung pinaghihinalaan kong na-take over ang account?",
      answer:
        "Kontakin agad ang support at palitan ang password. Maaaring pansamantalang limitahan ng platform ang account para protektahan ang assets at data.",
    },
  },
  "security.mfa": {
    "zh-Hans": {
      question: "支持多因素验证吗？",
      answer:
        "MFA 将逐步提供。启用后，提现、敏感设置与高风险登录会要求额外验证。",
    },
    id: {
      question: "Apakah autentikasi multi-faktor didukung?",
      answer:
        "MFA akan diluncurkan bertahap. Setelah aktif, penarikan, pengaturan sensitif, dan masuk berisiko tinggi memerlukan verifikasi tambahan.",
    },
    vi: {
      question: "Có hỗ trợ xác thực đa yếu tố không?",
      answer:
        "MFA sẽ được triển khai dần dần. Khi bật, rút tiền, cài đặt nhạy cảm và đăng nhập rủi ro cao sẽ cần xác minh bổ sung.",
    },
    tl: {
      question: "Suportado ba ang multi-factor authentication?",
      answer:
        "Unti-unting ilalabas ang MFA. Kapag enabled, withdrawals, sensitive settings, at high-risk sign-ins ay mangangailangan ng dagdag na verification.",
    },
  },
};

function item(
  id: string,
  order: number,
  zhQuestion: string,
  zhAnswer: string,
  enQuestion: string,
  enAnswer: string,
): FaqItem {
  return {
    id,
    order,
    enabled: true,
    question: { "zh-Hant": zhQuestion, en: enQuestion },
    answer: { "zh-Hant": zhAnswer, en: enAnswer },
  };
}

function defaultSectionsWithLocalizedCopy(): FaqSection[] {
  const sections = clone(defaultSections);

  for (const section of sections) {
    const titles = localizedSectionTitles[section.id];
    if (titles) {
      section.title = { ...section.title, ...titles };
    }

    for (const item of section.items) {
      const copy = localizedItemCopy[`${section.id}.${item.id}`];
      if (!copy) continue;

      for (const [locale, localizedCopy] of Object.entries(copy) as Array<
        [FaqLocale, { question: string; answer: string }]
      >) {
        item.question[locale] = localizedCopy.question;
        item.answer[locale] = localizedCopy.answer;
      }
    }
  }

  return sections;
}

export function createDefaultFaqCatalog(
  options: CreateDefaultFaqCatalogOptions = {},
): FaqCatalog {
  return normalizeFaqCatalog({
    schemaVersion: FAQ_SCHEMA_VERSION,
    updatedAt: options.updatedAt ?? new Date().toISOString(),
    updatedBy: options.updatedBy ?? "system:default-faq",
    sections: defaultSectionsWithLocalizedCopy(),
  });
}

export function backfillFaqCatalogMissingLocalizedText(
  catalog: FaqCatalog,
): FaqCatalog {
  const normalized = normalizeFaqCatalog(catalog);
  const fallback = createDefaultFaqCatalog({
    updatedAt: normalized.updatedAt,
    updatedBy: normalized.updatedBy,
  });

  return normalizeFaqCatalog({
    ...normalized,
    sections: normalized.sections.map((section) => {
      const fallbackSection = fallback.sections.find(
        (candidate) => candidate.id === section.id,
      );
      return {
        ...section,
        title: {
          ...(fallbackSection?.title ?? {}),
          ...section.title,
        },
        items: section.items.map((item) => {
          const fallbackItem = fallbackSection?.items.find(
            (candidate) => candidate.id === item.id,
          );
          return {
            ...item,
            question: {
              ...(fallbackItem?.question ?? {}),
              ...item.question,
            },
            answer: {
              ...(fallbackItem?.answer ?? {}),
              ...item.answer,
            },
          };
        }),
      };
    }),
  });
}

export function normalizeFaqCatalog(value: unknown): FaqCatalog {
  const record = expectRecord(value, "catalog");
  const schemaVersion = record.schemaVersion;
  if (schemaVersion !== FAQ_SCHEMA_VERSION) {
    throw new FaqCatalogValidationError("FAQ catalog schemaVersion must be 1");
  }

  const updatedAt = expectNonEmptyString(record.updatedAt, "updatedAt");
  if (Number.isNaN(Date.parse(updatedAt))) {
    throw new FaqCatalogValidationError(
      "FAQ catalog updatedAt must be an ISO timestamp",
    );
  }

  const updatedBy = expectNonEmptyString(record.updatedBy, "updatedBy");
  const rawSections = expectArray(record.sections, "sections");
  if (rawSections.length > maxSections) {
    throw new FaqCatalogValidationError(
      `FAQ catalog supports at most ${maxSections} sections`,
    );
  }

  const sectionIds = new Set<string>();
  const sections = rawSections.map((sectionValue, sectionIndex) => {
    const sectionRecord = expectRecord(
      sectionValue,
      `sections[${sectionIndex}]`,
    );
    const id = expectId(sectionRecord.id, `sections[${sectionIndex}].id`);
    if (sectionIds.has(id)) {
      throw new FaqCatalogValidationError(`Duplicate FAQ section id: ${id}`);
    }
    sectionIds.add(id);

    const title = normalizeLocalizedText(
      sectionRecord.title,
      `sections[${sectionIndex}].title`,
    );
    const enabled = expectBoolean(
      sectionRecord.enabled,
      `sections[${sectionIndex}].enabled`,
    );
    if (enabled && !localizedValue(title, FAQ_DEFAULT_LOCALE)) {
      throw new FaqCatalogValidationError(
        `Enabled FAQ section ${id} needs a default-locale title`,
      );
    }

    const rawItems = expectArray(
      sectionRecord.items,
      `sections[${sectionIndex}].items`,
    );
    if (rawItems.length > maxItemsPerSection) {
      throw new FaqCatalogValidationError(
        `FAQ section ${id} supports at most ${maxItemsPerSection} items`,
      );
    }

    const itemIds = new Set<string>();
    const items = rawItems.map((itemValue, itemIndex) => {
      const itemRecord = expectRecord(
        itemValue,
        `sections[${sectionIndex}].items[${itemIndex}]`,
      );
      const itemId = expectId(
        itemRecord.id,
        `sections[${sectionIndex}].items[${itemIndex}].id`,
      );
      if (itemIds.has(itemId)) {
        throw new FaqCatalogValidationError(
          `Duplicate FAQ item id in section ${id}: ${itemId}`,
        );
      }
      itemIds.add(itemId);

      const itemEnabled = expectBoolean(
        itemRecord.enabled,
        `sections[${sectionIndex}].items[${itemIndex}].enabled`,
      );
      const question = normalizeLocalizedText(
        itemRecord.question,
        `sections[${sectionIndex}].items[${itemIndex}].question`,
      );
      const answer = normalizeLocalizedText(
        itemRecord.answer,
        `sections[${sectionIndex}].items[${itemIndex}].answer`,
      );

      if (
        itemEnabled &&
        (!localizedValue(question, FAQ_DEFAULT_LOCALE) ||
          !localizedValue(answer, FAQ_DEFAULT_LOCALE))
      ) {
        throw new FaqCatalogValidationError(
          `Enabled FAQ item ${id}.${itemId} needs default-locale question and answer`,
        );
      }

      return {
        id: itemId,
        order: expectInteger(
          itemRecord.order,
          `sections[${sectionIndex}].items[${itemIndex}].order`,
        ),
        enabled: itemEnabled,
        question,
        answer,
      };
    });

    return {
      id,
      order: expectInteger(
        sectionRecord.order,
        `sections[${sectionIndex}].order`,
      ),
      enabled,
      title,
      items: items.sort(compareByOrderThenId),
    };
  });

  return {
    schemaVersion: FAQ_SCHEMA_VERSION,
    updatedAt,
    updatedBy,
    sections: sections.sort(compareByOrderThenId),
  };
}

export function faqCatalogToPublicSections(
  catalog: FaqCatalog,
  locale: FaqLocale,
): PublicFaqSection[] {
  const normalized = normalizeFaqCatalog(catalog);

  return normalized.sections
    .filter((section) => section.enabled)
    .map((section) => ({
      id: section.id,
      title: localizedValue(section.title, locale) ?? section.id,
      items: section.items
        .filter((item) => item.enabled)
        .map((item) => ({
          id: item.id,
          question: localizedValue(item.question, locale) ?? item.id,
          answer: localizedValue(item.answer, locale) ?? "",
        }))
        .filter((item) => item.question && item.answer),
    }))
    .filter((section) => section.title && section.items.length > 0);
}

export function serializeFaqCatalog(catalog: FaqCatalog): string {
  return JSON.stringify(normalizeFaqCatalog(catalog));
}

export function parseStoredFaqCatalog(value: string): FaqCatalog {
  try {
    return normalizeFaqCatalog(JSON.parse(value));
  } catch (error) {
    if (error instanceof FaqCatalogValidationError) {
      throw error;
    }
    throw new FaqCatalogValidationError("Stored FAQ catalog is not valid JSON");
  }
}

export class InMemoryFaqCatalogStore implements FaqCatalogStore {
  private catalog: FaqCatalog | null = null;

  async load(): Promise<FaqCatalog | null> {
    return this.catalog ? clone(this.catalog) : null;
  }

  async save(catalog: FaqCatalog): Promise<void> {
    this.catalog = normalizeFaqCatalog(catalog);
  }

  async reset(): Promise<void> {
    this.catalog = null;
  }
}

export class KvFaqCatalogStore implements FaqCatalogStore {
  private readonly kv: FaqKvNamespace;
  private readonly key: string;

  constructor(kv: FaqKvNamespace, options: KvFaqCatalogStoreOptions = {}) {
    this.kv = kv;
    this.key = options.key ?? DEFAULT_FAQ_KV_KEY;
  }

  async load(): Promise<FaqCatalog | null> {
    const stored = await this.kv.get(this.key);
    return stored ? parseStoredFaqCatalog(stored) : null;
  }

  async save(catalog: FaqCatalog): Promise<void> {
    await this.kv.put(this.key, serializeFaqCatalog(catalog));
  }

  async reset(): Promise<void> {
    await this.kv.delete(this.key);
  }
}

function localizedValue(
  text: FaqLocalizedText,
  locale: FaqLocale,
): string | undefined {
  return text[locale] ?? text[FAQ_DEFAULT_LOCALE] ?? text.en;
}

function normalizeLocalizedText(
  value: unknown,
  path: string,
): FaqLocalizedText {
  const record = expectRecord(value, path);
  const normalized: FaqLocalizedText = {};

  for (const locale of FAQ_SUPPORTED_LOCALES) {
    const raw = record[locale];
    if (raw === undefined || raw === null) continue;
    const text = expectString(raw, `${path}.${locale}`).trim();
    if (text.length > maxTextLength) {
      throw new FaqCatalogValidationError(
        `${path}.${locale} exceeds ${maxTextLength} characters`,
      );
    }
    if (text) {
      normalized[locale] = text;
    }
  }

  return normalized;
}

function expectRecord(value: unknown, path: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new FaqCatalogValidationError(`${path} must be an object`);
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new FaqCatalogValidationError(`${path} must be an array`);
  }
  return value;
}

function expectString(value: unknown, path: string): string {
  if (typeof value !== "string") {
    throw new FaqCatalogValidationError(`${path} must be a string`);
  }
  return value;
}

function expectNonEmptyString(value: unknown, path: string): string {
  const text = expectString(value, path).trim();
  if (!text) {
    throw new FaqCatalogValidationError(`${path} must not be empty`);
  }
  if (text.length > maxTextLength) {
    throw new FaqCatalogValidationError(
      `${path} exceeds ${maxTextLength} characters`,
    );
  }
  return text;
}

function expectId(value: unknown, path: string): string {
  const id = expectNonEmptyString(value, path);
  if (!idPattern.test(id)) {
    throw new FaqCatalogValidationError(
      `${path} must be 2-64 letters, numbers, underscores, or dashes`,
    );
  }
  return id;
}

function expectInteger(value: unknown, path: string): number {
  if (!Number.isInteger(value)) {
    throw new FaqCatalogValidationError(`${path} must be an integer`);
  }
  return value as number;
}

function expectBoolean(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") {
    throw new FaqCatalogValidationError(`${path} must be a boolean`);
  }
  return value;
}

function compareByOrderThenId<T extends { order: number; id: string }>(
  a: T,
  b: T,
): number {
  return a.order === b.order ? a.id.localeCompare(b.id) : a.order - b.order;
}

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
