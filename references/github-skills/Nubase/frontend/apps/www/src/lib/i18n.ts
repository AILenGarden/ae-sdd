export const LANGS = ['en', 'zh', 'ja', 'fr'] as const;
export type Lang = (typeof LANGS)[number];
export const DEFAULT_LANG: Lang = 'en';
export const LANG_COOKIE = 'nb_lang';

export const LANG_NAMES: Record<Lang, string> = {
  en: 'English',
  zh: '中文',
  ja: '日本語',
  fr: 'Français',
};

export interface Dict {
  hero: { badge: string; titleA: string; titleHighlight: string; titleEnd: string; subtitle: string; ctaPrimary: string; ctaSecondary: string };
  marquee: { label: string };
  flow: {
    eyebrow: string;
    title: string;
    subtitle: string;
    termHint: string;
    termC1: string;
    termC2: string;
    stages: { title: string; desc: string }[];
  };
  toolchain: { eyebrow: string; title: string; subtitle: string; stats: string[]; tags: { publishFrontend: string; deployLogic: string; firstClass: string }; bodies: string[] };
  integrations: { eyebrow: string; title: string; subtitle: string };
  cta: { title: string; body: string; button: string };
  learn: { eyebrow: string; title: string; cards: { title: string; body: string; cta: string }[]; trust: string[] };
  finalCta: { title: string; body: string; primary: string; secondary: string };
  nav: { items: string[]; star: string };
  footer: { tagline: string; cols: { title: string; links: string[] }[]; copyright: string; madeBy: string };
}

// Module bodies (toolchain cards) in order: Database, Auth, Storage, Assets, Functions, AI Gateway, Memory, cron.
const en: Dict = {
  hero: {
    badge: 'Free · Open source · Apache-2.0',
    titleA: 'Turn AI-written code into ',
    titleHighlight: 'real apps',
    titleEnd: '.',
    subtitle: 'Configure the plugin once — your coding agent ships the whole app online.',
    ctaPrimary: 'Get started free',
    ctaSecondary: 'Star on GitHub',
  },
  marquee: { label: 'Drive it from any AI coding agent' },
  flow: {
    eyebrow: 'From prompt to production',
    title: 'Generate → live, in four moves',
    subtitle: 'Connect the plugin once — then your agent walks the whole path, using the right modules at each stage.',
    termHint: 'configure once — then ship from the chat',
    termC1: '# 1 · connect your agent (Claude Code / Codex)',
    termC2: '# 2 · then just ask — it deploys for you',
    stages: [
      { title: 'Model the data', desc: 'Tables with RLS, users, and files.' },
      { title: 'Ship the backend', desc: 'Edge functions, model routing, memory.' },
      { title: 'Publish the frontend', desc: 'Your generated UI on a public CDN.' },
      { title: 'Go live', desc: 'Scheduled jobs, running on their own.' },
    ],
  },
  toolchain: {
    eyebrow: 'Everything in one service',
    title: 'Eight modules to ship an app',
    subtitle: 'Data and identity, a place to publish the frontend, edge functions for backend logic, scheduled jobs, an AI gateway and durable memory — the same per-project token model everywhere, isolation by default.',
    stats: ['modules in one service', 'command to connect your agent', 'platform: frontend + backend + cron', 'vendor lock-in · Apache-2.0'],
    tags: { publishFrontend: 'publish frontend', deployLogic: 'deploy logic', firstClass: 'first-class' },
    bodies: [
      'An isolated PostgreSQL per project with a PostgREST-compatible REST API, Row Level Security and JWT claims.',
      'Supabase-style auth, per project: email, OAuth, magic links, MFA / TOTP, OTP and anonymous sign-in.',
      'S3 / R2-compatible object storage with public & private buckets, signed URLs and policy-aware access.',
      'Publish the generated frontend to a public CDN at /assets/v1 — your agent uploads HTML/CSS/JS and gets a live URL. No separate static host.',
      'Deploy AI-written backend logic as edge functions at /functions/v1 — per-function secrets, logs, verify_jwt, on a local or Cloudflare runtime.',
      'Bring your own model. One gateway, OpenAI- and Anthropic-compatible, with per-project keys and usage tracking.',
      'A real memory layer for AI apps. Facts are extracted, embedded and recalled with a hybrid engine — not a bolted-on vector script.',
      'Schedule recurring jobs — invoke an edge function or a database function on a crontab, run by the control plane with run history.',
    ],
  },
  integrations: {
    eyebrow: 'Plays well with your stack',
    title: 'Use your favorite tools',
    subtitle: 'Nubase speaks the protocols you already use — OpenAI & Anthropic APIs, PostgREST-style REST, S3 storage, a Workers runtime and MCP for coding agents.',
  },
  cta: {
    title: 'Not sure where to start?',
    body: 'Connect the plugin and tell your agent to build something — you’ll have a live frontend, backend functions and a database running in minutes.',
    button: 'Read the quickstart',
  },
  learn: {
    eyebrow: 'Learn, explore, grow',
    title: 'Everything you need to ship',
    cards: [
      { title: 'Deploy an app', body: 'Generate → live: model the data, deploy Functions, publish the frontend to Assets and schedule cron — all from your agent.', cta: 'Read the guide' },
      { title: 'Community', body: 'Star the repo, open issues, and shape the roadmap. Nubase is Apache-2.0 and built in the open.', cta: 'Join on GitHub' },
      { title: 'Self-host', body: 'One Docker image bundles Postgres, Redis, the API and Studio. Run unlimited projects on your own box.', cta: 'See the architecture' },
    ],
    trust: ['Isolation by default', 'Multi-project control plane', 'Bring your own model', 'Apache-2.0, free forever'],
  },
  finalCta: {
    title: 'Turn AI-written code into a real app — in one command.',
    body: 'Free, open and self-hosted under Apache-2.0. Connect the plugin once, then ship the frontend, backend and cron from your coding agent.',
    primary: 'Get started free',
    secondary: 'Star on GitHub',
  },
  nav: { items: ['Features', 'Compare', 'Docs', 'Blog', 'News'], star: 'Star' },
  footer: {
    tagline: 'Turn AI-written code into real apps — your coding agent ships the frontend, backend and cron online. Self-host the whole stack in one Docker image.',
    cols: [
      { title: 'Product', links: ['Features', 'Compare', 'Documentation'] },
      { title: 'Developers', links: ['Quickstart', 'Architecture', 'Memory'] },
      { title: 'Resources', links: ['Blog', 'News', 'GitHub'] },
      { title: 'Legal', links: ['Privacy', 'Terms'] },
    ],
    copyright: 'Apache-2.0. Built for AI-native apps.',
    madeBy: 'Made with care by the Nubase team.',
  },
};

const zh: Dict = {
  hero: {
    badge: '免费 · 开源 · Apache-2.0',
    titleA: '让 AI 写的代码变成',
    titleHighlight: '真正上线的应用',
    titleEnd: '。',
    subtitle: '配置一次插件 —— 你的编码 Agent 就能把整个应用发布上线。',
    ctaPrimary: '免费开始',
    ctaSecondary: 'GitHub 加星',
  },
  marquee: { label: '任意 AI 编码工具都能驱动' },
  flow: {
    eyebrow: '从一句话到上线',
    title: '生成 → 上线,只需四步',
    subtitle: '只配置一次插件 —— Agent 就会走完整条路径,在每个阶段用上对应的模块。',
    termHint: '配置一次 —— 之后在对话里发布',
    termC1: '# 1 · 连接你的 Agent(Claude Code / Codex)',
    termC2: '# 2 · 然后直接说一句 —— 它替你部署',
    stages: [
      { title: '建模数据', desc: '带 RLS 的数据表、用户与文件。' },
      { title: '部署后端', desc: '边缘函数、模型路由、记忆。' },
      { title: '发布前端', desc: '把生成的界面发到公共 CDN。' },
      { title: '正式上线', desc: '定时任务自动运行。' },
    ],
  },
  toolchain: {
    eyebrow: '一个服务,全部齐备',
    title: '八个模块,把应用送上线',
    subtitle: '数据与身份、发布前端的地方、承载后端逻辑的边缘函数、定时任务、AI 网关和持久记忆 —— 处处共用同一套按项目的令牌模型,默认隔离。',
    stats: ['个模块,一个服务', '条命令连接 Agent', '个平台:前端 + 后端 + 定时', '厂商锁定 · Apache-2.0'],
    tags: { publishFrontend: '发布前端', deployLogic: '部署逻辑', firstClass: '一等公民' },
    bodies: [
      '每个项目一个独立的 PostgreSQL,提供 PostgREST 兼容的 REST API、行级安全(RLS)与 JWT 声明。',
      '按项目的 Supabase 风格鉴权:邮箱、OAuth、魔法链接、MFA/TOTP、OTP 与匿名登录。',
      '兼容 S3 / R2 的对象存储,支持公共与私有桶、签名 URL 和策略感知的访问控制。',
      '把生成的前端发布到 /assets/v1 的公共 CDN —— Agent 上传 HTML/CSS/JS 即得到可访问的 URL,无需另购静态托管。',
      '把 AI 写的后端逻辑部署为 /functions/v1 的边缘函数 —— 支持每函数密钥、日志、verify_jwt,可跑在本地或 Cloudflare 运行时。',
      '自带模型即可。一个网关,同时兼容 OpenAI 与 Anthropic,带按项目的密钥与用量统计。',
      '为 AI 应用打造的真正记忆层:抽取、向量化并用混合引擎召回事实 —— 而不是临时拼凑的向量脚本。',
      '编排定时任务 —— 按 crontab 调用边缘函数或数据库函数,由控制面执行并保留运行历史。',
    ],
  },
  integrations: {
    eyebrow: '与你的技术栈无缝配合',
    title: '继续用你顺手的工具',
    subtitle: 'Nubase 说的是你已经在用的协议 —— OpenAI 与 Anthropic API、PostgREST 风格的 REST、S3 存储、Workers 运行时,以及面向编码 Agent 的 MCP。',
  },
  cta: {
    title: '不知道从哪开始?',
    body: '连接插件,让 Agent 去做点东西 —— 几分钟内你就能拥有一个可访问的前端、后端函数和一个数据库。',
    button: '阅读快速上手',
  },
  learn: {
    eyebrow: '学习 · 探索 · 成长',
    title: '上线所需,一应俱全',
    cards: [
      { title: '部署一个应用', body: '生成 → 上线:建模数据、部署 Functions、把前端发到 Assets、配置 cron —— 全部由 Agent 完成。', cta: '阅读指南' },
      { title: '社区', body: '给仓库加星、提 issue、共同塑造路线图。Nubase 基于 Apache-2.0,完全开放开发。', cta: '在 GitHub 加入' },
      { title: '自托管', body: '一个 Docker 镜像打包了 Postgres、Redis、API 和 Studio。在自己的机器上运行无限个项目。', cta: '查看架构' },
    ],
    trust: ['默认隔离', '多项目控制面', '自带模型', 'Apache-2.0,永久免费'],
  },
  finalCta: {
    title: '一条命令,把 AI 写的代码变成真正的应用。',
    body: '基于 Apache-2.0,免费、开放、可自托管。配置一次插件,之后由编码 Agent 发布前端、后端与定时任务。',
    primary: '免费开始',
    secondary: 'GitHub 加星',
  },
  nav: { items: ['功能', '对比', '文档', '博客', '动态'], star: 'Star' },
  footer: {
    tagline: '让 AI 写的代码变成真正的应用 —— 编码 Agent 把前端、后端与定时任务一并发布上线。用一个 Docker 镜像自托管整套栈。',
    cols: [
      { title: '产品', links: ['功能', '对比', '文档'] },
      { title: '开发者', links: ['快速上手', '架构', '记忆'] },
      { title: '资源', links: ['博客', '动态', 'GitHub'] },
      { title: '法律', links: ['隐私', '条款'] },
    ],
    copyright: 'Apache-2.0。为 AI 原生应用而生。',
    madeBy: '由 Nubase 团队用心打造。',
  },
};

const ja: Dict = {
  hero: {
    badge: '無料 · オープンソース · Apache-2.0',
    titleA: 'AI が書いたコードを',
    titleHighlight: '本物のアプリ',
    titleEnd: 'に変える。',
    subtitle: 'プラグインを一度設定するだけ —— コーディングエージェントがアプリ全体をオンラインに公開します。',
    ctaPrimary: '無料で始める',
    ctaSecondary: 'GitHub でスター',
  },
  marquee: { label: 'どの AI コーディングエージェントからでも操作' },
  flow: {
    eyebrow: 'プロンプトから本番へ',
    title: '生成 → 公開まで、4 ステップ',
    subtitle: 'プラグインを一度つなぐだけ —— あとはエージェントが各段階で適切なモジュールを使い、最後まで進めます。',
    termHint: '一度設定 —— あとはチャットから公開',
    termC1: '# 1 · エージェントを接続(Claude Code / Codex)',
    termC2: '# 2 · あとは頼むだけ —— 自動でデプロイ',
    stages: [
      { title: 'データを設計', desc: 'RLS 付きテーブル、ユーザー、ファイル。' },
      { title: 'バックエンドを配備', desc: 'エッジ関数、モデルルーティング、メモリ。' },
      { title: 'フロントを公開', desc: '生成した UI を公開 CDN へ。' },
      { title: '本番稼働', desc: 'スケジュールジョブが自動で動く。' },
    ],
  },
  toolchain: {
    eyebrow: 'すべてを 1 つのサービスで',
    title: 'アプリを公開する 8 つのモジュール',
    subtitle: 'データと認証、フロント公開の場、バックエンド用のエッジ関数、定期ジョブ、AI ゲートウェイ、永続メモリ —— どこでもプロジェクト単位の同じトークンモデル、既定で分離。',
    stats: ['モジュールを 1 サービスに', 'コマンドでエージェント接続', 'プラットフォーム:フロント+バック+定期', 'ベンダーロックイン · Apache-2.0'],
    tags: { publishFrontend: 'フロント公開', deployLogic: 'ロジック配備', firstClass: '第一級' },
    bodies: [
      'プロジェクトごとに独立した PostgreSQL。PostgREST 互換の REST API、行レベルセキュリティ(RLS)、JWT クレームを備えます。',
      'プロジェクト単位の Supabase 風認証:メール、OAuth、マジックリンク、MFA/TOTP、OTP、匿名サインイン。',
      'S3 / R2 互換のオブジェクトストレージ。公開・非公開バケット、署名付き URL、ポリシー対応アクセス。',
      '生成したフロントを /assets/v1 の公開 CDN へ公開 —— エージェントが HTML/CSS/JS をアップすると公開 URL が得られます。別途の静的ホスティング不要。',
      'AI が書いたバックエンドロジックを /functions/v1 のエッジ関数として配備 —— 関数ごとのシークレット、ログ、verify_jwt。ローカルまたは Cloudflare ランタイムで動作。',
      '好きなモデルを持ち込み可能。OpenAI と Anthropic 互換の単一ゲートウェイ、プロジェクト単位のキーと使用量計測。',
      'AI アプリのための本物のメモリ層。事実を抽出・埋め込み、ハイブリッドエンジンで想起 —— 後付けのベクトルスクリプトではありません。',
      '定期ジョブをスケジュール —— crontab でエッジ関数や DB 関数を呼び出し、コントロールプレーンが実行・履歴を保持。',
    ],
  },
  integrations: {
    eyebrow: '既存スタックとよく馴染む',
    title: '使い慣れたツールのままで',
    subtitle: 'Nubase は既に使っているプロトコルで話します —— OpenAI と Anthropic API、PostgREST 風 REST、S3 ストレージ、Workers ランタイム、コーディングエージェント向け MCP。',
  },
  cta: {
    title: 'どこから始めるか迷ったら?',
    body: 'プラグインをつないでエージェントに何か作らせてみてください —— 数分で公開フロント、バックエンド関数、データベースが動きます。',
    button: 'クイックスタートを読む',
  },
  learn: {
    eyebrow: '学び、探り、伸ばす',
    title: '公開に必要なものすべて',
    cards: [
      { title: 'アプリをデプロイ', body: '生成 → 公開:データ設計、Functions 配備、Assets へフロント公開、cron 設定 —— すべてエージェントから。', cta: 'ガイドを読む' },
      { title: 'コミュニティ', body: 'リポジトリにスター、issue を立て、ロードマップを一緒に。Nubase は Apache-2.0 でオープンに開発。', cta: 'GitHub で参加' },
      { title: 'セルフホスト', body: '1 つの Docker イメージに Postgres・Redis・API・Studio を同梱。自分のマシンで無制限のプロジェクトを。', cta: 'アーキテクチャを見る' },
    ],
    trust: ['既定で分離', 'マルチプロジェクト制御', 'モデル持ち込み可', 'Apache-2.0、ずっと無料'],
  },
  finalCta: {
    title: '一行のコマンドで、AI のコードを本物のアプリに。',
    body: 'Apache-2.0 の下で無料・オープン・セルフホスト。一度設定すれば、あとはエージェントがフロント・バック・cron を公開します。',
    primary: '無料で始める',
    secondary: 'GitHub でスター',
  },
  nav: { items: ['機能', '比較', 'ドキュメント', 'ブログ', 'ニュース'], star: 'Star' },
  footer: {
    tagline: 'AI が書いたコードを本物のアプリに —— エージェントがフロント・バック・cron をまとめて公開。1 つの Docker イメージでスタック全体をセルフホスト。',
    cols: [
      { title: 'プロダクト', links: ['機能', '比較', 'ドキュメント'] },
      { title: '開発者', links: ['クイックスタート', 'アーキテクチャ', 'メモリ'] },
      { title: 'リソース', links: ['ブログ', 'ニュース', 'GitHub'] },
      { title: '法務', links: ['プライバシー', '利用規約'] },
    ],
    copyright: 'Apache-2.0。AI ネイティブアプリのために。',
    madeBy: 'Nubase チームが心を込めて。',
  },
};

const fr: Dict = {
  hero: {
    badge: 'Gratuit · Open source · Apache-2.0',
    titleA: 'Transformez le code écrit par l’IA en ',
    titleHighlight: 'de vraies applis',
    titleEnd: '.',
    subtitle: 'Configurez le plugin une fois — votre agent de code met toute l’appli en ligne.',
    ctaPrimary: 'Commencer gratuitement',
    ctaSecondary: 'Star sur GitHub',
  },
  marquee: { label: 'Pilotez-le depuis n’importe quel agent de code IA' },
  flow: {
    eyebrow: 'Du prompt à la production',
    title: 'Générer → en ligne, en quatre étapes',
    subtitle: 'Branchez le plugin une fois — l’agent parcourt ensuite tout le chemin, en utilisant les bons modules à chaque étape.',
    termHint: 'configurez une fois — puis livrez depuis le chat',
    termC1: '# 1 · connectez votre agent (Claude Code / Codex)',
    termC2: '# 2 · puis demandez — il déploie pour vous',
    stages: [
      { title: 'Modéliser les données', desc: 'Tables avec RLS, utilisateurs et fichiers.' },
      { title: 'Livrer le backend', desc: 'Edge functions, routage de modèles, mémoire.' },
      { title: 'Publier le frontend', desc: 'Votre UI générée sur un CDN public.' },
      { title: 'Mettre en ligne', desc: 'Des tâches planifiées, en autonomie.' },
    ],
  },
  toolchain: {
    eyebrow: 'Tout dans un seul service',
    title: 'Huit modules pour livrer une appli',
    subtitle: 'Données et identité, un endroit pour publier le frontend, des edge functions pour la logique backend, des tâches planifiées, une passerelle IA et une mémoire durable — partout le même modèle de jeton par projet, isolation par défaut.',
    stats: ['modules en un service', 'commande pour connecter l’agent', 'plateforme : frontend + backend + cron', 'enfermement propriétaire · Apache-2.0'],
    tags: { publishFrontend: 'publier le front', deployLogic: 'déployer la logique', firstClass: 'natif' },
    bodies: [
      'Un PostgreSQL isolé par projet, avec une API REST compatible PostgREST, la sécurité au niveau ligne (RLS) et des claims JWT.',
      'Auth façon Supabase, par projet : e-mail, OAuth, magic links, MFA / TOTP, OTP et connexion anonyme.',
      'Stockage objet compatible S3 / R2 avec buckets publics et privés, URLs signées et accès tenant compte des politiques.',
      'Publiez le frontend généré sur un CDN public à /assets/v1 — l’agent téléverse du HTML/CSS/JS et obtient une URL en ligne. Pas d’hébergement statique séparé.',
      'Déployez la logique backend écrite par l’IA en edge functions à /functions/v1 — secrets par fonction, logs, verify_jwt, sur un runtime local ou Cloudflare.',
      'Apportez votre modèle. Une passerelle, compatible OpenAI et Anthropic, avec des clés par projet et un suivi d’usage.',
      'Une vraie couche mémoire pour les applis IA. Les faits sont extraits, vectorisés et rappelés par un moteur hybride — pas un script vectoriel bricolé.',
      'Planifiez des tâches récurrentes — invoquez une edge function ou une fonction de base de données via crontab, exécutées par le control plane avec un historique.',
    ],
  },
  integrations: {
    eyebrow: 'S’intègre à votre stack',
    title: 'Gardez vos outils préférés',
    subtitle: 'Nubase parle les protocoles que vous utilisez déjà — APIs OpenAI et Anthropic, REST façon PostgREST, stockage S3, un runtime Workers et MCP pour les agents de code.',
  },
  cta: {
    title: 'Pas sûr par où commencer ?',
    body: 'Branchez le plugin et demandez à votre agent de construire quelque chose — vous aurez un frontend en ligne, des fonctions backend et une base de données en quelques minutes.',
    button: 'Lire le démarrage rapide',
  },
  learn: {
    eyebrow: 'Apprendre, explorer, grandir',
    title: 'Tout pour livrer',
    cards: [
      { title: 'Déployer une appli', body: 'Générer → en ligne : modéliser les données, déployer les Functions, publier le frontend sur Assets et planifier cron — le tout depuis votre agent.', cta: 'Lire le guide' },
      { title: 'Communauté', body: 'Mettez une étoile, ouvrez des issues et façonnez la roadmap. Nubase est en Apache-2.0, développé au grand jour.', cta: 'Rejoindre sur GitHub' },
      { title: 'Auto-hébergement', body: 'Une image Docker réunit Postgres, Redis, l’API et Studio. Faites tourner un nombre illimité de projets chez vous.', cta: 'Voir l’architecture' },
    ],
    trust: ['Isolation par défaut', 'Control plane multi-projets', 'Apportez votre modèle', 'Apache-2.0, gratuit à vie'],
  },
  finalCta: {
    title: 'Transformez le code de l’IA en vraie appli — en une commande.',
    body: 'Gratuit, ouvert et auto-hébergé sous Apache-2.0. Configurez le plugin une fois, puis livrez frontend, backend et cron depuis votre agent de code.',
    primary: 'Commencer gratuitement',
    secondary: 'Star sur GitHub',
  },
  nav: { items: ['Fonctionnalités', 'Comparer', 'Docs', 'Blog', 'Actus'], star: 'Star' },
  footer: {
    tagline: 'Transformez le code de l’IA en vraies applis — votre agent met le frontend, le backend et cron en ligne. Auto-hébergez toute la stack dans une image Docker.',
    cols: [
      { title: 'Produit', links: ['Fonctionnalités', 'Comparer', 'Documentation'] },
      { title: 'Développeurs', links: ['Démarrage rapide', 'Architecture', 'Mémoire'] },
      { title: 'Ressources', links: ['Blog', 'Actus', 'GitHub'] },
      { title: 'Légal', links: ['Confidentialité', 'Conditions'] },
    ],
    copyright: 'Apache-2.0. Conçu pour les applis IA-natives.',
    madeBy: 'Fait avec soin par l’équipe Nubase.',
  },
};

const DICTS: Record<Lang, Dict> = { en, zh, ja, fr };

export function getDict(lang: Lang): Dict {
  return DICTS[lang] ?? en;
}
