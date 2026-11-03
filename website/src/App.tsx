import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode, SVGProps } from "react";
import { ScrollThumb } from "./ScrollThumb";
import { useLocale } from "./i18n/I18nProvider";
import { catalogs, supportedLocales } from "./i18n/catalogs";
import type { Locale } from "./i18n/catalogs";

const repository = (
  import.meta.env.VITE_REPOSITORY_URL as string | undefined
)?.replace(/\/$/, "");
const asset = (name: string) => `${import.meta.env.BASE_URL}${name}`;

function Icon({
  name,
  ...props
}: SVGProps<SVGSVGElement> & {
  name:
    | "chevron"
    | "copy"
    | "check"
    | "keyboard"
    | "trackpad"
    | "mouse"
    | "shield"
    | "globe";
}) {
  const paths: Record<string, ReactNode> = {
    globe: (
      <>
        <circle cx="12" cy="12" r="9" />
        <ellipse cx="12" cy="12" rx="4" ry="9" />
        <path d="M3 12h18" />
      </>
    ),
    chevron: <path d="m9 5 7 7-7 7" />,
    copy: (
      <>
        <rect x="8" y="8" width="12" height="13" rx="2" />
        <path d="M15 8V3H3v13h5" />
      </>
    ),
    check: <path d="m5 12 4 4L19 6" />,
    keyboard: (
      <>
        <rect x="2" y="5" width="20" height="14" rx="2" />
        <path d="M5 9h1m3 0h1m3 0h1m3 0h2M5 12h1m3 0h1m3 0h1m3 0h2M7 16h10" />
      </>
    ),
    trackpad: (
      <>
        <rect x="3" y="3" width="18" height="18" rx="3" />
        <path d="M8 14V9m4 5V7m4 7V9m-6 8 2-2 2 2" />
      </>
    ),
    mouse: (
      <>
        <rect x="6" y="2" width="12" height="20" rx="6" />
        <path d="M12 2v8M6 10h12" />
      </>
    ),
    shield: (
      <>
        <path d="m12 2 8 3v6c0 5-8 11-8 11S4 16 4 11V5Z" />
        <path d="m8 11 3 3 5-6" />
      </>
    ),
  };
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.65"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...props}
    >
      {paths[name]}
    </svg>
  );
}

function useMotionPreference() {
  const [systemReduced, setSystemReduced] = useState(
    () => window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  );
  const [manualReduced, setManualReduced] = useState(() => {
    try {
      return localStorage.getItem("whalu-reduced-motion") === "true";
    } catch {
      return false;
    }
  });
  const reducedMotion = systemReduced || manualReduced;
  useEffect(() => {
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setSystemReduced(media.matches);
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  useLayoutEffect(() => {
    document.documentElement.dataset.motion = reducedMotion
      ? "reduced"
      : "full";
    return () => {
      delete document.documentElement.dataset.motion;
    };
  }, [reducedMotion]);
  function toggleMotion() {
    const next = !manualReduced;
    setManualReduced(next);
    try {
      localStorage.setItem("whalu-reduced-motion", String(next));
    } catch {
      /* The preference still works for this visit. */
    }
  }
  return { reducedMotion, systemReduced, toggleMotion };
}

function ProductDemo({ reducedMotion }: { reducedMotion: boolean }) {
  const { t } = useLocale();
  const video = useRef<HTMLVideoElement>(null);
  const userPaused = useRef(false);

  useEffect(() => {
    const node = video.current;
    if (!node) return;
    let visible = false;
    let automaticPause = false;
    const update = () => {
      if (!visible || document.hidden || reducedMotion) {
        if (!node.paused) {
          automaticPause = true;
          node.pause();
        }
      } else if (!userPaused.current) {
        void node.play().catch(() => {
          // Native controls remain available when autoplay is blocked.
        });
      }
    };
    const onPause = () => {
      if (automaticPause) automaticPause = false;
      else userPaused.current = true;
    };
    const onPlay = () => {
      userPaused.current = false;
    };
    const observer = new IntersectionObserver(
      ([entry]) => {
        visible = entry.isIntersecting && entry.intersectionRatio >= 0.2;
        update();
      },
      { threshold: 0.2 },
    );
    node.addEventListener("pause", onPause);
    node.addEventListener("play", onPlay);
    document.addEventListener("visibilitychange", update);
    observer.observe(node);
    return () => {
      observer.disconnect();
      node.removeEventListener("pause", onPause);
      node.removeEventListener("play", onPlay);
      document.removeEventListener("visibilitychange", update);
    };
  }, [reducedMotion]);

  return (
    <div className="product-demo">
      <div className="laptop">
        <div className="laptop-display">
          <div className="camera-notch" aria-hidden="true">
            <i />
          </div>
          <div className="demo-screen" id="demo">
            <video
              ref={video}
              className="demo-video"
              width="1920"
              height="1248"
              controls
              muted
              loop
              playsInline
              preload="metadata"
              poster={asset("media/whalu-demo-poster.jpg")}
              aria-label={t("demo.label")}
            >
              <source src={asset("media/whalu-demo.mp4")} type="video/mp4" />
              <a href={asset("media/whalu-demo.mp4")}>
                {t("demo.download")}
              </a>
            </video>
          </div>
        </div>
        <div className="laptop-base" aria-hidden="true">
          <span />
        </div>
      </div>
    </div>
  );
}

function HeroBackdrop() {
  const backdrop = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const node = backdrop.current;
    const svg = node?.querySelector("svg");
    if (!node || !svg) return;
    let visible = false;
    let scrolling = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const update = () => {
      svg.style.animationPlayState =
        visible && !scrolling ? "running" : "paused";
    };
    const observer = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting;
      update();
    });
    const onScroll = () => {
      if (!scrolling) {
        scrolling = true;
        update();
      }
      clearTimeout(timer);
      timer = setTimeout(() => {
        scrolling = false;
        update();
      }, 180);
    };
    observer.observe(node);
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      observer.disconnect();
      clearTimeout(timer);
      window.removeEventListener("scroll", onScroll);
    };
  }, []);
  const id = useId().replace(/:/g, "");
  return (
    <div ref={backdrop} className="hero-backdrop" aria-hidden="true">
      <svg viewBox="0 0 1440 1000" preserveAspectRatio="xMidYMid slice">
        <defs>
          <linearGradient id={`${id}-edge`} x1="0" y1="0" x2="1" y2="1">
            <stop stopColor="#a8d8ff" stopOpacity="0" />
            <stop offset=".35" stopColor="#c6e5ff" stopOpacity=".8" />
            <stop offset=".6" stopColor="#559be7" stopOpacity=".35" />
            <stop offset="1" stopColor="#3468b6" stopOpacity="0" />
          </linearGradient>
          <linearGradient id={`${id}-glass`} x1="0" y1="0" x2="1" y2=".3">
            <stop stopColor="#20344f" stopOpacity="0" />
            <stop offset=".8" stopColor="#5d9bd8" stopOpacity=".12" />
            <stop offset="1" stopColor="#c1defb" stopOpacity=".22" />
          </linearGradient>
        </defs>
        <g
          transform="rotate(-32 720 500)"
          fill={`url(#${id}-glass)`}
          stroke={`url(#${id}-edge)`}
        >
          <rect
            x="750"
            y="-300"
            width="550"
            height="1050"
            rx="52"
            strokeWidth="14"
            opacity=".08"
          />
          <rect
            x="750"
            y="-300"
            width="550"
            height="1050"
            rx="52"
            strokeWidth="1.5"
          />
          <rect
            x="840"
            y="-220"
            width="550"
            height="1050"
            rx="52"
            strokeWidth="1"
            opacity=".6"
          />
          <rect
            x="-80"
            y="520"
            width="480"
            height="750"
            rx="48"
            strokeWidth="10"
            opacity=".08"
          />
          <rect
            x="-80"
            y="520"
            width="480"
            height="750"
            rx="48"
            strokeWidth="1"
            opacity=".65"
          />
        </g>
      </svg>
    </div>
  );
}

function Preview({ reducedMotion }: { reducedMotion: boolean }) {
  const { t } = useLocale();
  const section = useRef<HTMLElement>(null);
  useEffect(() => {
    const node = section.current;
    if (!node || reducedMotion) return;
    let animation: Animation | undefined;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (!entry.isIntersecting) return;
        observer.disconnect();
        animation = node.querySelector(".product-demo")?.animate(
          [
            { opacity: 0.3, transform: "translateY(36px) scale(.97)" },
            { opacity: 1, transform: "translateY(0) scale(1)" },
          ],
          { duration: 850, easing: "cubic-bezier(.2,.7,.2,1)" },
        );
      },
      { threshold: 0.12 },
    );
    observer.observe(node);
    return () => {
      observer.disconnect();
      animation?.cancel();
    };
  }, [reducedMotion]);
  return (
    <section
      ref={section}
      className="preview-section"
      id="how-it-works"
      tabIndex={-1}
      aria-labelledby="preview-heading"
    >
      <div className="preview-inner">
        <h2 id="preview-heading">{t("hero.learn")}</h2>
        <ProductDemo reducedMotion={reducedMotion} />
      </div>
    </section>
  );
}

const shortcutGroups = [
  {
    id: "navigate",
    rows: [
      ["shortcuts.previousNext", ["⌃", "⌥", "↑ ↓ ← →"]],
      ["shortcuts.next", ["⌘", "Tab"]],
      ["shortcuts.previous", ["⌘", "`"]],
    ],
  },
  {
    id: "arrange",
    rows: [
      ["shortcuts.reorder", ["⌃", "⌥", "⌘", "↑ ↓"]],
      ["shortcuts.space", ["⌃", "⌥", "⌘", "← →"]],
      ["shortcuts.tile", ["⌃", "⌘", "← →"]],
    ],
  },
] as const;

function Controls() {
  const { t } = useLocale();
  const methods = ["keyboard", "trackpad", "mouse"] as const;
  return (
    <section
      className="controls-section section-pad"
      id="controls"
      aria-labelledby="controls-heading"
    >
      <div className="section-heading">
        <h2 id="controls-heading">{t("controls.title")}</h2>
      </div>
      <div className="control-methods">
        {methods.map((method) => (
          <article key={method}>
            <Icon name={method} />
            <h3>{t(`controls.${method}.name`)}</h3>
            <p>{t(`controls.${method}.description`)}</p>
          </article>
        ))}
      </div>
      <div className="shortcut-sheet">
        {shortcutGroups.map((group) => (
          <div className={`shortcut-group shortcut-${group.id}`} key={group.id}>
            <h3>{t(`shortcuts.${group.id}`)}</h3>
            {group.rows.map(([label, keys]) => (
              <div className="shortcut-row" key={label}>
                <span>{t(label)}</span>
                <span className="key-combination">
                  {keys.map((key) => (
                    <kbd key={key}>{key}</kbd>
                  ))}
                </span>
              </div>
            ))}
          </div>
        ))}
      </div>
      <p className="shortcut-note">{t("shortcuts.note")}</p>
    </section>
  );
}

function Install() {
  const { t } = useLocale();
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  async function copy() {
    try {
      await navigator.clipboard.writeText("./scripts/install.sh");
      setCopied(true);
      setError(false);
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), 2500);
    } catch {
      setError(true);
    }
  }
  return (
    <section className="install-section section-pad" id="get-whalu">
      <div className="install-heading">
        <h2>{t("nav.get")}</h2>
        <span className="compatibility">
          {t("install.compatibility", { version: "0.5.1" })}
        </span>
      </div>
      <div className="install-guide">
        <div>
          <h3>{t("install.build")}</h3>
          <p>
            {repository ? (
              <>
                {t("install.cloneBefore")}{" "}
                <a href={repository} target="_blank" rel="noreferrer">
                  {t("install.repository")}
                </a>{" "}
                {t("install.cloneAfter")}
              </>
            ) : (
              t("install.local")
            )}
          </p>
          <div className="install-command">
            <code>./scripts/install.sh</code>
            <button
              onClick={copy}
              aria-label={t(copied ? "install.copiedLabel" : "install.copy")}
            >
              <Icon name={copied ? "check" : "copy"} />
            </button>
          </div>
          <small role="status">
            {copied
              ? t("install.copied")
              : error
                ? t("install.copyError")
                : t("install.requires")}
          </small>
        </div>
        <div>
          <h3>{t("install.permissions")}</h3>
          <p>{t("install.permissionsText")}</p>
          <p className="privacy-note">
            <Icon name="shield" />
            {t("privacy.description")}
          </p>
        </div>
      </div>
      {repository && (
        <a
          className="text-link"
          href={repository}
          target="_blank"
          rel="noreferrer"
        >
          {t("install.source")} <Icon name="chevron" />
        </a>
      )}
    </section>
  );
}

function LanguageSelector() {
  const { locale, setLocale, t } = useLocale();
  const language = catalogs[locale];
  return (
    <div className="language-switcher">
      <Icon name="globe" />
      <span className="language-name" aria-hidden="true">
        {language.nativeName}
      </span>
      <span className="language-code" aria-hidden="true">
        {language.shortName}
      </span>
      <Icon name="chevron" className="language-chevron" />
      <select
        value={locale}
        aria-label={t("language.label")}
        title={t("language.label")}
        onChange={(event) => {
          const value = event.currentTarget.value as Locale;
          if (supportedLocales.includes(value)) setLocale(value);
        }}
      >
        {supportedLocales.map((code) => (
          <option key={code} value={code} lang={code}>
            {catalogs[code].nativeName}
          </option>
        ))}
      </select>
    </div>
  );
}

function ProductNav() {
  const { t } = useLocale();
  const header = useRef<HTMLElement>(null);
  useLayoutEffect(() => {
    const node = header.current;
    if (!node) return;
    const update = () => {
      const scrolled = String(window.scrollY > 1);
      if (node.dataset.scrolled !== scrolled) node.dataset.scrolled = scrolled;
    };
    update();
    window.addEventListener("scroll", update, { passive: true });
    return () => window.removeEventListener("scroll", update);
  }, []);
  return (
    <header ref={header} className="product-nav" data-scrolled="false">
      <div>
        <a className="brand" href="#" aria-label={t("nav.home")}>
          <img src={asset("whalu-icon.png")} width="28" height="28" alt="" />
          <span>Whalu</span>
        </a>
        <div className="nav-actions">
          <nav aria-label={t("nav.label")}>
            <a href="#controls">{t("nav.controls")}</a>
            {repository && (
              <a href={repository} target="_blank" rel="noreferrer">
                GitHub ↗
              </a>
            )}
          </nav>
          <LanguageSelector />
          <a className="button button-small" href="#get-whalu">
            {t("nav.get")}
          </a>
        </div>
      </div>
    </header>
  );
}

export function App() {
  useLayoutEffect(() => {
    const navigation = performance.getEntriesByType("navigation")[0] as
      | PerformanceNavigationTiming
      | undefined;
    if (navigation?.type === "back_forward") return;
    if (!location.hash) {
      window.scrollTo({ top: 0, behavior: "instant" });
      return;
    }
    // React creates section targets after the browser's initial anchor lookup.
    try {
      document
        .getElementById(decodeURIComponent(location.hash.slice(1)))
        ?.scrollIntoView({ block: "start", behavior: "instant" });
    } catch {
      /* Ignore malformed fragment identifiers. */
    }
  }, []);
  const { t } = useLocale();
  const { reducedMotion, systemReduced, toggleMotion } = useMotionPreference();
  return (
    <>
      <a className="skip-link" href="#main">
        {t("skip")}
      </a>
      <ProductNav />
      <ScrollThumb />
      <main id="main" tabIndex={-1}>
        <section className="hero" aria-labelledby="hero-heading">
          <HeroBackdrop />
          <div className="hero-layout">
            <div className="hero-copy">
              <p className="hero-kicker">{t("hero.kicker")}</p>
              <h1 id="hero-heading">
                {t("hero.title")}
                <br />
                <span>{t("hero.titleAccent")}</span>
              </h1>
              <p className="hero-description">{t("hero.description")}</p>
              <div className="hero-actions">
                <a className="button" href="#get-whalu">
                  {t("nav.get")}
                </a>
                <a className="text-link" href="#how-it-works">
                  {t("hero.learn")} <Icon name="chevron" />
                </a>
              </div>
              <p className="hero-availability">{t("hero.availability")}</p>
            </div>
          </div>
        </section>
        <Preview reducedMotion={reducedMotion} />
        <Controls />
        <Install />
      </main>
      <footer className="site-footer">
        <div>
          <div className="footer-bottom">
            <a href="#" className="footer-brand">
              Whalu
            </a>
            <span>{t("footer.license")}</span>
            <a href={asset("THIRD_PARTY_NOTICES.txt")}>{t("footer.notices")}</a>
            <button
              className="motion-preference"
              onClick={toggleMotion}
              aria-pressed={reducedMotion}
              disabled={systemReduced}
              title={systemReduced ? t("motion.systemHint") : undefined}
            >
              <span aria-hidden="true">{reducedMotion ? "◉" : "◌"}</span>
              {systemReduced
                ? t("motion.system")
                : reducedMotion
                  ? t("motion.enable")
                  : t("motion.reduce")}
            </button>
            <a href="#" className="back-top">
              {t("footer.top")}
            </a>
          </div>
        </div>
      </footer>
    </>
  );
}
