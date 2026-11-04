import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent, ReactNode, SVGProps } from "react";
import { ScrollThumb } from "./ScrollThumb";
import { useLocale } from "./i18n/I18nProvider";
import { catalogs, supportedLocales } from "./i18n/catalogs";

const repository = (
  (import.meta.env.VITE_REPOSITORY_URL as string | undefined) ||
  "https://github.com/danileph/whalu"
).replace(/\/$/, "");
const latestRelease = `${repository}/releases/latest`;
const download = `${latestRelease}/download/Whalu-macOS-universal.zip`;
const brewInstall = "brew install --cask danileph/whalu/whalu";
const asset = (name: string) => `${import.meta.env.BASE_URL}${name}`;

function Icon({
  name,
  ...props
}: SVGProps<SVGSVGElement> & {
  name:
    | "chevron"
    | "copy"
    | "check"
    | "download"
    | "github"
    | "chip"
    | "archive"
    | "shield"
    | "keyboard"
    | "trackpad"
    | "mouse"
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
    download: <path d="M12 3v12m-5-5 5 5 5-5M3 16v4a1 1 0 0 0 1 1h16a1 1 0 0 0 1-1v-4" />,
    github: (
      <path
        fill="currentColor"
        stroke="none"
        transform="scale(1.5)"
        d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82a7.65 7.65 0 0 1 4 0c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8 8 0 0 0 16 8c0-4.42-3.58-8-8-8Z"
      />
    ),
    chip: (
      <>
        <rect x="5" y="5" width="14" height="14" rx="2" />
        <path d="M8 2v3m4-3v3m4-3v3M8 19v3m4-3v3m4-3v3M2 8h3m-3 4h3m-3 4h3m14-8h3m-3 4h3m-3 4h3" />
      </>
    ),
    archive: <path d="M6 3h9l4 4v14H5V4a1 1 0 0 1 1-1Zm6 0v8m-2 0h4m-5 5h6" />,
    shield: <path d="m12 2 8 3v6c0 5-4 8-8 11-4-3-8-6-8-11V5l8-3Zm-3 10 2 2 4-4" />,
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
        <h2 id="preview-heading">{t("demo.title")}</h2>
        <ProductDemo reducedMotion={reducedMotion} />
      </div>
    </section>
  );
}

const shortcutGroups = [
  {
    id: "navigate",
    rows: [
      ["shortcuts.overview", [["⌃", "⌥"]]],
      ["shortcuts.previous", [["⌃", "⌥", "↑"], ["⌃", "⌥", "K"]]],
      ["shortcuts.next", [["⌃", "⌥", "↓"], ["⌃", "⌥", "J"]]],
      ["shortcuts.appPrevious", [["⌃", "⌥", "←"], ["⌃", "⌥", "H"]]],
      ["shortcuts.appNext", [["⌃", "⌥", "→"], ["⌃", "⌥", "L"]]],
      ["shortcuts.next", [["⌘", "Tab"]]],
      ["shortcuts.previous", [["⌘", "`"]]],
      ["shortcuts.lastUsed", [["⌘", "Esc"]]],
    ],
  },
  {
    id: "tile",
    rows: [
      ["shortcuts.left", [["⌃", "⌘", "←"], ["⌃", "⌘", "H"]]],
      ["shortcuts.right", [["⌃", "⌘", "→"], ["⌃", "⌘", "L"]]],
      ["shortcuts.top", [["⌃", "⌘", "↑"], ["⌃", "⌘", "K"]]],
      ["shortcuts.bottom", [["⌃", "⌘", "↓"], ["⌃", "⌘", "J"]]],
      ["shortcuts.fill", [["⌃", "⌘", "M"]]],
      ["shortcuts.center", [["⌃", "⌘", "I"]]],
      ["shortcuts.restore", [["⌃", "⌘", "N"]]],
    ],
  },
  {
    id: "stack",
    rows: [
      ["shortcuts.stackUp", [["⌃", "⌥", "⌘", "↑"], ["⌃", "⌥", "⌘", "K"]]],
      ["shortcuts.stackDown", [["⌃", "⌥", "⌘", "↓"], ["⌃", "⌥", "⌘", "J"]]],
      ["shortcuts.spacePrevious", [["⌃", "⌥", "⌘", "←"], ["⌃", "⌥", "⌘", "H"]]],
      ["shortcuts.spaceNext", [["⌃", "⌥", "⌘", "→"], ["⌃", "⌥", "⌘", "L"]]],
    ],
  },
  {
    id: "arrange",
    rows: [
      ["shortcuts.leftRight", [["⌃", "⇧", "⌘", "←"], ["⌃", "⇧", "⌘", "H"]]],
      ["shortcuts.rightLeft", [["⌃", "⇧", "⌘", "→"], ["⌃", "⇧", "⌘", "L"]]],
      ["shortcuts.topBottom", [["⌃", "⇧", "⌘", "↑"], ["⌃", "⇧", "⌘", "K"]]],
      ["shortcuts.bottomTop", [["⌃", "⇧", "⌘", "↓"], ["⌃", "⇧", "⌘", "J"]]],
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
      <p className="shortcut-intro">{t("shortcuts.intro")}</p>
      <div className="shortcut-sheet">
        {shortcutGroups.map((group) => (
          <div className={`shortcut-group shortcut-${group.id}`} key={group.id}>
            <h3>{t(`shortcuts.${group.id}`)}</h3>
            {group.rows.map(([label, combinations], rowIndex) => (
              <div className="shortcut-row" key={`${label}-${rowIndex}`}>
                <span>{t(label)}</span>
                <span className="shortcut-alternatives">
                  {combinations.map((keys, index) => (
                    <span className="shortcut-alternative" key={keys.join("")}>
                      {index > 0 && <span className="shortcut-or">{t("shortcuts.or")}</span>}
                      <span className="key-combination">
                        {keys.map((key) => (
                          <kbd key={key}>{key}</kbd>
                        ))}
                      </span>
                    </span>
                  ))}
                </span>
              </div>
            ))}
          </div>
        ))}
      </div>
      <p className="shortcut-legend">{t("shortcuts.legend")}</p>
      <p className="shortcut-note">{t("shortcuts.note")}</p>
    </section>
  );
}

function Install() {
  const { t } = useLocale();
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied" | "error">("idle");
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  async function copy() {
    try {
      await navigator.clipboard.writeText(brewInstall);
      clearTimeout(timer.current);
      setCopyStatus("copied");
      timer.current = setTimeout(() => setCopyStatus("idle"), 2500);
    } catch {
      clearTimeout(timer.current);
      setCopyStatus("error");
    }
  }
  const copied = copyStatus === "copied";
  return (
    <section
      className="install-section section-pad"
      id="get-whalu"
      aria-labelledby="install-title"
    >
      <h2 id="install-title">{t("install.title")}</h2>
      <p className="download-intro">{t("download.intro")}</p>
      <div className="download-actions">
        <a className="button download-button" href={download}>
          <Icon name="download" />
          <span>
            {t("nav.get")}
            <small>macOS 15+</small>
          </span>
        </a>
        <div className="install-command">
          <code>{brewInstall}</code>
          <button
            type="button"
            onClick={copy}
            aria-label={t(copied ? "install.copiedLabel" : "install.copy")}
          >
            <Icon name={copied ? "check" : "copy"} />
          </button>
        </div>
      </div>
      <p className="copy-status" role="status">
        {copied ? t("install.copied") : copyStatus === "error" ? t("install.copyError") : ""}
      </p>
      <ul className="download-facts" role="list">
        <li><Icon name="chip" /> {t("install.compatibility")}</li>
        <li><Icon name="archive" /> {t("download.format")}</li>
        <li><Icon name="shield" /> {t("download.signing")}</li>
      </ul>
      <div className="install-links">
        <a className="text-link" href={latestRelease}>
          {t("download.releases")}
        </a>
        <a className="text-link" href={repository}>
          <Icon name="github" />
          {t("download.github")}
        </a>
      </div>
    </section>
  );
}

function LanguageSelector() {
  const { locale, preference, setPreference, t } = useLocale();
  const language = catalogs[locale];
  const [open, setOpen] = useState(false);
  const menuId = useId();
  const picker = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    if (open) {
      const selected = menu.current?.querySelector<HTMLButtonElement>('[aria-pressed="true"]');
      selected?.focus({ preventScroll: true });
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const dismiss = (event: PointerEvent) => {
      const node = picker.current;
      if (node && !event.composedPath().includes(node)) setOpen(false);
    };
    document.addEventListener("pointerdown", dismiss);
    return () => document.removeEventListener("pointerdown", dismiss);
  }, [open]);

  function closeAndFocusTrigger() {
    setOpen(false);
    trigger.current?.focus({ preventScroll: true });
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape" && open) {
      event.preventDefault();
      closeAndFocusTrigger();
      return;
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;

    event.preventDefault();
    if (!open) {
      setOpen(true);
      return;
    }
    const options = Array.from(menu.current?.querySelectorAll("button") ?? []);
    const current = options.findIndex((option) => option === event.target);
    const direction = event.key === "ArrowDown" ? 1 : -1;
    const next = current === -1 ? 0 : (current + direction + options.length) % options.length;
    options[next]?.focus({ preventScroll: true });
  }

  return (
    <div
      className="language-switcher"
      ref={picker}
      onKeyDown={onKeyDown}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false);
      }}
    >
      <button
        className="language-trigger"
        type="button"
        ref={trigger}
        aria-label={`${t("language.label")}, ${language.nativeName}`}
        aria-expanded={open}
        aria-controls={menuId}
        onClick={() => setOpen(!open)}
      >
        <Icon name="globe" />
        <span className="language-name" aria-hidden="true">{language.nativeName}</span>
        <span className="language-code" aria-hidden="true">{language.shortName}</span>
        <Icon name="chevron" className="language-chevron" />
      </button>
      <div
        className="language-menu"
        id={menuId}
        ref={menu}
        role="group"
        aria-label={t("language.label")}
        hidden={!open}
      >
        {(["auto", ...supportedLocales] as const).map((code) => (
          <button
            className="language-option"
            type="button"
            key={code}
            lang={code === "auto" ? locale : code}
            aria-pressed={preference === code}
            onClick={() => {
              setPreference(code);
              closeAndFocusTrigger();
            }}
          >
            {code === "auto" ? t("language.browser") : catalogs[code].nativeName}
            {preference === code && <Icon name="check" />}
          </button>
        ))}
      </div>
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
          <a className="button button-small" href={download}>
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
