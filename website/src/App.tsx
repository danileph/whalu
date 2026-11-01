import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type {
  CSSProperties,
  KeyboardEvent,
  PointerEvent,
  ReactNode,
  SVGProps,
} from "react";
import { useLocale } from "./i18n/I18nProvider";
import { catalogs, supportedLocales } from "./i18n/catalogs";
import type { Locale } from "./i18n/catalogs";

const repository = (
  import.meta.env.VITE_REPOSITORY_URL as string | undefined
)?.replace(/\/$/, "");
const asset = (name: string) => `${import.meta.env.BASE_URL}${name}`;
const clamp = (value: number) => Math.max(0, Math.min(1, value));
const illustrationDate = new Date("2026-11-01T10:24:00Z");

function Icon({
  name,
  ...props
}: SVGProps<SVGSVGElement> & {
  name:
    | "arrow"
    | "chevron"
    | "copy"
    | "check"
    | "plus"
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
    arrow: <path d="M4 12h16m-6-6 6 6-6 6" />,
    chevron: <path d="m9 5 7 7-7 7" />,
    copy: (
      <>
        <rect x="8" y="8" width="12" height="13" rx="2" />
        <path d="M15 8V3H3v13h5" />
      </>
    ),
    check: <path d="m5 12 4 4L19 6" />,
    plus: <path d="M12 5v14M5 12h14" />,
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

// Read layout once per frame. Motion only updates compositor-friendly CSS variables;
// React doesn't render on each scroll event, and the listener sleeps between events.
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

function useScrollMotion(reducedMotion: boolean, locale: Locale) {
  const revealed = useRef(new WeakSet<Element>());
  useEffect(() => {
    const scenes = [
      ...document.querySelectorAll<HTMLElement>("[data-scroll-scene]"),
    ];
    const reveals = [
      ...document.querySelectorAll<HTMLElement>("[data-reveal]"),
    ];
    const revealAnimations = new Set<Animation>();
    let frame = 0;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries)
          if (entry.isIntersecting) {
            observer.unobserve(entry.target);
            // Content stays visible before observation, including screenshots,
            // restored scroll positions, and browsers that skip an entry.
            if (
              !reducedMotion &&
              !revealed.current.has(entry.target) &&
              typeof entry.target.animate === "function"
            ) {
              const animation = entry.target.animate(
                [
                  { opacity: 0, transform: "translateY(32px)" },
                  { opacity: 1, transform: "translateY(0)" },
                ],
                { duration: 700, easing: "cubic-bezier(.2,.65,.3,1)" },
              );
              revealAnimations.add(animation);
              animation.onfinish = animation.oncancel = () =>
                revealAnimations.delete(animation);
            }
            revealed.current.add(entry.target);
          }
      },
      { threshold: 0 },
    );
    reveals.forEach((node) => observer.observe(node));

    function update() {
      frame = 0;
      const height = window.innerHeight;
      const measurements = scenes.map((node) => ({
        node,
        rect: node.getBoundingClientRect(),
      }));
      for (const { node, rect } of measurements) {
        const pinned = node.hasAttribute("data-pinned");
        const progress = reducedMotion
          ? 0.5
          : pinned
            ? clamp(-rect.top / Math.max(1, rect.height - height))
            : clamp((height - rect.top) / (height + rect.height));
        node.style.setProperty("--progress", progress.toFixed(4));
        if (pinned) {
          node.dataset.focus = progress > 0.52 ? "3" : "2";
          const chapter = progress < 0.34 ? "0" : progress < 0.7 ? "1" : "2";
          if (node.dataset.chapter !== chapter) {
            node.dataset.chapter = chapter;
            node
              .querySelectorAll<HTMLElement>("[data-chapter-button]")
              .forEach((button, i) =>
                button.setAttribute(
                  "aria-pressed",
                  String(i === Number(chapter)),
                ),
              );
          }
        }
      }
    }
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(update);
    };
    update();
    window.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      revealAnimations.forEach((animation) => animation.cancel());
      window.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
    };
  }, [reducedMotion, locale]);
}

function Wallpaper() {
  const id = useId().replace(/:/g, "");
  return (
    <svg
      className="wallpaper"
      viewBox="0 0 1440 900"
      preserveAspectRatio="xMidYMid slice"
      aria-hidden="true"
    >
      <defs>
        <linearGradient id={`${id}-sky`} x1="0" x2="1" y1="0" y2="1">
          <stop stopColor="#051f67" />
          <stop offset=".48" stopColor="#1474ca" />
          <stop offset="1" stopColor="#73d6e9" />
        </linearGradient>
        <linearGradient id={`${id}-wave`} x1=".2" x2=".8" y1="0" y2="1">
          <stop stopColor="#84e1f3" />
          <stop offset=".48" stopColor="#218ee0" />
          <stop offset="1" stopColor="#15369e" />
        </linearGradient>
        <linearGradient id={`${id}-edge`} x1="0" x2="1">
          <stop stopColor="#091a52" />
          <stop offset=".5" stopColor="#3068ce" />
          <stop offset="1" stopColor="#d4f8fc" />
        </linearGradient>
        <linearGradient id={`${id}-warm`} x1="0" x2="1" y1="1" y2="0">
          <stop stopColor="#e15534" />
          <stop offset=".4" stopColor="#ffb078" />
          <stop offset="1" stopColor="#f9e5be" />
        </linearGradient>
      </defs>
      <rect width="1440" height="900" fill={`url(#${id}-sky)`} />
      <path
        d="M0 900V590C270 765 440 53 837 47c360-5 249 560 603 570v283Z"
        fill={`url(#${id}-edge)`}
      />
      <path
        d="M-120 900C278 1070 345 182 824 151c282-18 300 394 616 391v358Z"
        fill={`url(#${id}-wave)`}
      />
      <path
        d="M529 900c170-89 275-372 418-414s295 143 493 118v296Z"
        fill={`url(#${id}-warm)`}
      />
      <path
        d="M-80 900c490 137 406-654 913-693 341-26 333 352 607 327"
        fill="none"
        stroke="#c0f4ff"
        strokeOpacity=".48"
        strokeWidth="2"
      />
    </svg>
  );
}

const apps = [
  {
    id: "finder",
    glyph: "⌑",
    color: "#2589ed",
  },
  {
    id: "editor",
    glyph: "⌘",
    color: "#4669df",
  },
  {
    id: "safari",
    glyph: "↗",
    color: "#007aff",
  },
  {
    id: "notes",
    glyph: "≡",
    color: "#e6ad25",
  },
  {
    id: "terminal",
    glyph: ">_",
    color: "#333336",
  },
] as const;
type AppId = (typeof apps)[number]["id"];

function AppBadge({ index }: { index: number }) {
  const app = apps[index];
  return (
    <span
      className={`app-badge app-${app.id}`}
      style={{ "--app-color": app.color } as CSSProperties}
      aria-hidden="true"
    >
      {app.glyph}
    </span>
  );
}

function WindowBody({ id }: { id: AppId }) {
  const { t, formatDate, formatNumber } = useLocale();
  if (id === "safari")
    return (
      <div className="safari-body">
        <aside>
          <span className="atlas-logo">a.</span>
          <span>{t("atlas.workspace")}</span>
          <b>◈ {t("atlas.overview")}</b>
          <span>▦ {t("atlas.projects")}</span>
          <span>◷ {t("atlas.timeline")}</span>
          <span>☷ {t("atlas.resources")}</span>
          <div className="avatar-row">
            <i>J</i>
            <i>A</i>
            <i>M</i>
            <small>{t("atlas.team")}</small>
          </div>
        </aside>
        <div className="atlas-main">
          <div className="atlas-top">
            <span>{t("atlas.breadcrumb")}</span>
            <span>◉　⋯</span>
          </div>
          <div className="atlas-heading">
            <div>
              <small>{t("atlas.kicker")}</small>
              <h4>{t("apps.safari.title")}</h4>
              <p>{t("atlas.subtitle")}</p>
            </div>
            <span className="project-tag">{t("atlas.progress")}</span>
          </div>
          <div className="atlas-art">
            <div className="chrome-orbit orbit-one" />
            <div className="chrome-orbit orbit-two" />
            <div className="chrome-orbit orbit-three" />
            <span>{t("atlas.art")}</span>
            <small>{t("atlas.artCaption")}</small>
          </div>
          <div className="atlas-tasks">
            <span>
              <i className="task-dot blue" /> {t("atlas.research")}{" "}
              <b>{t("atlas.complete")}</b>
            </span>
            <span>
              <i className="task-dot purple" /> {t("atlas.design")}{" "}
              <b>{t("atlas.progress")}</b>
            </span>
            <span>
              <i className="task-dot green" /> {t("atlas.development")}{" "}
              <b>{t("atlas.next")}</b>
            </span>
          </div>
        </div>
      </div>
    );
  if (id === "notes")
    return (
      <div className="notes-body">
        <aside>
          <span>{t("apps.notes.name")}</span>
          <b>
            {t("notes.all")} <i>{formatNumber(12)}</i>
          </b>
          <span>
            {t("notes.projects")} <i>{formatNumber(5)}</i>
          </span>
          <span>
            {t("notes.personal")} <i>{formatNumber(7)}</i>
          </span>
        </aside>
        <div>
          <time dateTime={illustrationDate.toISOString()}>
            {formatDate(illustrationDate, {
              dateStyle: "long",
              timeStyle: "short",
            })}
          </time>
          <h4>{t("notes.title")}</h4>
          <p>{t("notes.intro")}</p>
          <ul>
            <li>
              <i className="checked">✓</i> {t("notes.first")}
            </li>
            <li>
              <i className="checked">✓</i> {t("notes.second")}
            </li>
            <li>
              <i /> {t("notes.third")}
            </li>
          </ul>
          <div className="note-rule" />
          <p className="note-quiet">{t("notes.quiet")}</p>
        </div>
      </div>
    );
  if (id === "finder")
    return (
      <div className="finder-body">
        <aside>
          <small>{t("finder.favorites")}</small>
          <span>◷ {t("finder.recents")}</span>
          <span>⌂ {t("finder.desktop")}</span>
          <b>▱ {t("finder.projects")}</b>
          <span>↓ {t("finder.downloads")}</span>
          <span>☁ iCloud Drive</span>
        </aside>
        <div className="folders">
          {(
            [
              "Whalu",
              "design",
              "photography",
              "weekend",
              "archive",
              "resources",
            ] as const
          ).map((name) => (
            <div key={name}>
              <i className="folder" />
              <span>{name === "Whalu" ? name : t(`finder.${name}`)}</span>
            </div>
          ))}
        </div>
      </div>
    );
  if (id === "editor")
    return (
      <div className="editor-body">
        <aside>
          <small>{t("editor.explorer")}</small>
          <span>⌄ WHALU</span>
          <span>　⌄ src</span>
          <b>　　main.rs</b>
          <span>　　navigation.rs</span>
          <span>　Cargo.toml</span>
        </aside>
        <div className="code-pane">
          <span className="code-tab">main.rs　×</span>
          <pre>
            <span className="code-comment">// {t("editor.comment")}</span>
            {"\n\n"}
            <span className="code-pink">fn</span>{" "}
            <span className="code-blue">main</span>() {"{"}
            {"\n"} <span className="code-pink">let</span> desktop =
            Desktop::current();{"\n\n"} <span className="code-pink">for</span>{" "}
            window <span className="code-pink">in</span> desktop.windows() {"{"}
            {"\n"} window.focus();{"\n"} {"}"}
            {"\n"}
            {"}"}
          </pre>
          <div className="code-status">
            main* <span>Rust　 UTF-8</span>
          </div>
        </div>
      </div>
    );
  return (
    <div className="terminal-body">
      <p>
        <span>~/projects/whalu</span> on main
      </p>
      <p>❯ cargo build --release</p>
      <p className="terminal-dim">　 Compiling whalu v0.5.1</p>
      <p>
        <span>　 Finished</span> release profile
      </p>
      <p>
        ❯ <i />
      </p>
    </div>
  );
}

function MacWindow({
  index,
  className = "",
  style,
}: {
  index: number;
  className?: string;
  style?: CSSProperties;
}) {
  const { t } = useLocale();
  return (
    <div className={`mac-window ${className}`} style={style} aria-hidden="true">
      <div className="window-bar">
        <div className="traffic-lights">
          <i />
          <i />
          <i />
        </div>
        {apps[index].id === "safari" ? (
          <span className="address-bar">
            ◈　 atlas.local <span>↻</span>
          </span>
        ) : (
          <span>{t(`apps.${apps[index].id}.title`)}</span>
        )}
        <span className="window-tools">⊞</span>
      </div>
      <WindowBody id={apps[index].id} />
    </div>
  );
}

function ProductDemo() {
  const { t, formatDate, formatNumber } = useLocale();
  const [selected, setSelected] = useState(2);
  const wheelTime = useRef(0);
  const touchStart = useRef<{ x: number; y: number } | null>(null);
  const swiped = useRef(false);
  const select = (index: number) =>
    setSelected(Math.max(0, Math.min(apps.length - 1, index)));
  function keyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    if (
      ![
        "ArrowDown",
        "ArrowUp",
        "ArrowRight",
        "ArrowLeft",
        "j",
        "k",
        "Home",
        "End",
      ].includes(event.key)
    )
      return;
    event.preventDefault();
    event.currentTarget.focus({ preventScroll: true });
    if (event.key === "Home") return select(0);
    if (event.key === "End") return select(apps.length - 1);
    select(
      (selected +
        (["ArrowDown", "ArrowRight", "j"].includes(event.key) ? 1 : -1) +
        apps.length) %
        apps.length,
    );
  }
  function endSwipe(event: PointerEvent<HTMLDivElement>) {
    if (!touchStart.current) return;
    const x = event.clientX - touchStart.current.x;
    const y = event.clientY - touchStart.current.y;
    touchStart.current = null;
    if (Math.abs(x) > 38 && Math.abs(x) > Math.abs(y) * 1.3) {
      swiped.current = true;
      select(selected + (x < 0 ? 1 : -1));
    }
  }
  return (
    <div className="product-demo" id="demo">
      <div className="laptop">
        <div className="laptop-display">
          <div className="camera-notch">
            <i />
          </div>
          <div
            className="mac-desktop"
            tabIndex={0}
            role="region"
            aria-label={t("demo.label")}
            aria-describedby="demo-help"
            onKeyDown={keyDown}
            onPointerDown={(event) => {
              swiped.current = false;
              if (event.pointerType === "touch")
                touchStart.current = { x: event.clientX, y: event.clientY };
            }}
            onClickCapture={(event) => {
              if (swiped.current) {
                event.preventDefault();
                event.stopPropagation();
                swiped.current = false;
              }
            }}
            onPointerUp={endSwipe}
            onPointerCancel={() => {
              touchStart.current = null;
            }}
          >
            <Wallpaper />
            <div className="menu-bar" aria-hidden="true">
              <span>
                ●　 <b>{t(`apps.${apps[selected].id}.name`)}</b>　{" "}
                {t("demo.menu")}
              </span>
              <span>
                W　 ◒　{" "}
                {formatDate(illustrationDate, {
                  weekday: "short",
                  hour: "numeric",
                  minute: "2-digit",
                })}
              </span>
            </div>
            <MacWindow index={3} className="desktop-back-window" />
            <MacWindow
              index={selected}
              className="desktop-main-window"
              key={selected}
            />
            <div
              className="desktop-wheel"
              onWheel={(event) => {
                if (
                  Math.abs(event.deltaY) < 8 ||
                  Date.now() - wheelTime.current < 220
                )
                  return;
                wheelTime.current = Date.now();
                select(selected + Math.sign(event.deltaY));
              }}
            >
              {apps.map((app, index) => {
                const distance = index - selected;
                return (
                  <button
                    key={app.id}
                    className={`thumbnail${distance === 0 ? " selected" : ""}`}
                    style={
                      {
                        "--distance": distance,
                        "--depth": Math.abs(distance),
                        zIndex: 10 - Math.abs(distance),
                      } as CSSProperties
                    }
                    aria-label={t("demo.switch", {
                      app: t(`apps.${app.id}.name`),
                    })}
                    aria-pressed={distance === 0}
                    tabIndex={distance === 0 ? 0 : -1}
                    onClick={() => select(index)}
                  >
                    <MacWindow index={index} />
                    <AppBadge index={index} />
                  </button>
                );
              })}
            </div>
            <div className="dock" aria-hidden="true">
              {apps.map((app, index) => (
                <AppBadge index={index} key={app.id} />
              ))}
              <span />
              <img src={asset("whalu-icon.png")} alt="" />
            </div>
          </div>
        </div>
        <div className="laptop-base">
          <span />
        </div>
        <div className="laptop-shadow" />
      </div>
      <div className="demo-controls">
        <span>{t("demo.try")}</span>
        <div className="demo-apps" role="group" aria-label={t("demo.choose")}>
          {apps.map((app, index) => (
            <button
              key={app.id}
              aria-label={t("demo.show", { app: t(`apps.${app.id}.name`) })}
              aria-pressed={selected === index}
              onClick={() => select(index)}
            >
              {t(`apps.${app.id}.name`)}
            </button>
          ))}
        </div>
        <span className="demo-selection" role="status" aria-atomic="true">
          {t(`apps.${apps[selected].id}.name`)}{" "}
          <span>
            {formatNumber(selected + 1, { minimumIntegerDigits: 2 })} /{" "}
            {formatNumber(apps.length, { minimumIntegerDigits: 2 })}
          </span>
        </span>
      </div>
      <p className="demo-disclaimer" id="demo-help">
        <span className="desktop-demo-hint">{t("demo.desktopHint")}</span>
        <span className="touch-demo-hint">{t("demo.touchHint")}</span>
        <span className="demo-illustration-label">
          {t("demo.illustration")}
        </span>
      </p>
    </div>
  );
}

const chapters = ["windows", "focus", "order"] as const;

function ScrollStory() {
  const { t, formatNumber } = useLocale();
  const ref = useRef<HTMLElement>(null);
  function goToChapter(index: number) {
    const section = ref.current;
    if (!section) return;
    const range = Math.max(1, section.offsetHeight - window.innerHeight);
    const reduced = document.documentElement.dataset.motion === "reduced";
    window.scrollTo({
      top:
        section.getBoundingClientRect().top +
        window.scrollY +
        range * [0.08, 0.5, 0.91][index],
      behavior: reduced ? "instant" : "smooth",
    });
  }
  return (
    <section
      className="scroll-story"
      id="overview"
      ref={ref}
      data-scroll-scene
      data-pinned
      data-chapter="0"
      data-focus="2"
      aria-label={t("story.label")}
    >
      <div className="story-sticky">
        <div className="story-inner">
          <div className="story-copy">
            <p className="eyebrow">{t("story.kicker")}</p>
            <div className="story-chapters">
              {chapters.map((chapter, index) => (
                <div
                  className={`chapter chapter-${index}`}
                  id={`overview-chapter-${index}`}
                  key={chapter}
                >
                  <h2>
                    {t(`story.${chapter}.title`)}
                    <br />
                    <span>{t(`story.${chapter}.accent`)}</span>
                  </h2>
                  <p>{t(`story.${chapter}.text`)}</p>
                </div>
              ))}
            </div>
            <div className="story-scroll-cue">
              <span /> {t("story.scroll")}
            </div>
          </div>
          <div className="story-stage" aria-hidden="true">
            <div className="story-glow" />
            <div className="story-stack">
              {apps.map((app, index) => {
                const distance = index - 2;
                const depth = Math.abs(distance);
                return (
                  <div
                    className={`story-window story-window-${index}`}
                    key={app.id}
                    style={
                      {
                        "--slot": distance,
                        "--pile-x": `${distance * 34}px`,
                        "--pile-y": `${distance * -22}px`,
                        "--layer": 10 - depth,
                      } as CSSProperties
                    }
                  >
                    <MacWindow index={index} />
                    <AppBadge index={index} />
                  </div>
                );
              })}
            </div>
          </div>
          <div
            className="story-navigation"
            role="group"
            aria-label={t("story.navigation")}
          >
            {chapters.map((chapter, index) => (
              <button
                key={chapter}
                data-chapter-button
                aria-controls={`overview-chapter-${index}`}
                aria-pressed={index === 0}
                onClick={() => goToChapter(index)}
              >
                <span>
                  {formatNumber(index + 1, { minimumIntegerDigits: 2 })}
                </span>
                {t(`story.${chapter}.label`)}
              </button>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

const shortcutGroups = [
  {
    id: "navigate",
    rows: [
      ["shortcuts.previousNext", "⌃ ⌥ ↑ / ↓"],
      ["shortcuts.sameApp", "⌃ ⌥ ← / →"],
      ["shortcuts.next", "⌘ Tab"],
    ],
  },
  {
    id: "arrange",
    rows: [
      ["shortcuts.reorder", "⌃ ⌥ ⌘ ↑ / ↓"],
      ["shortcuts.space", "⌃ ⌥ ⌘ ← / →"],
      ["shortcuts.tile", "⌃ ⌘ ← / →"],
    ],
  },
] as const;

function Controls() {
  const { t } = useLocale();
  const [open, setOpen] = useState(false);
  const [method, setMethod] = useState(0);
  function onTabKey(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? 2
          : (index + (event.key === "ArrowRight" ? 1 : -1) + 3) % 3;
    setMethod(next);
    event.currentTarget.parentElement
      ?.querySelectorAll<HTMLButtonElement>('[role="tab"]')
      [next]?.focus();
  }
  const methods = ["keyboard", "trackpad", "mouse"] as const;

  return (
    <section className="controls-section section-pad" id="controls">
      <div className="section-heading" data-reveal>
        <p className="eyebrow">{t("controls.kicker")}</p>
        <h2>
          {t("controls.title")}
          <br />
          <span>{t("controls.accent")}</span>
        </h2>
      </div>
      <div className="controls-card" data-reveal>
        <div className="input-visual" data-method={method} aria-hidden="true">
          <div className="keyboard-model">
            <div className="keyboard-top-row">
              {["fn", "control", "option", "command"].map((key) => (
                <span key={key}>
                  <i>
                    {key === "control"
                      ? "⌃"
                      : key === "option"
                        ? "⌥"
                        : key === "command"
                          ? "⌘"
                          : "fn"}
                  </i>
                  <small>{key}</small>
                </span>
              ))}
            </div>
            <div className="arrow-key-row">
              <span>←</span>
              <div>
                <span className="key-up">↑</span>
                <span className="key-down">↓</span>
              </div>
              <span>→</span>
            </div>
          </div>
          <div className="trackpad-model">
            <div className="finger-trails">
              <i />
              <i />
              <i />
            </div>
            <span>↑</span>
          </div>
          <div className="mouse-model">
            <span />
            <i />
          </div>
        </div>
        <div className="controls-copy">
          <div
            className="method-tabs"
            role="tablist"
            aria-label={t("controls.methodLabel")}
          >
            {methods.map((item, index) => (
              <button
                key={item}
                role="tab"
                id={`method-${index}`}
                aria-controls="method-panel"
                aria-selected={method === index}
                tabIndex={method === index ? 0 : -1}
                onClick={() => setMethod(index)}
                onKeyDown={(event) => onTabKey(event, index)}
              >
                <Icon name={item} />
                {t(`controls.${item}.name`)}
              </button>
            ))}
          </div>
          <div
            role="tabpanel"
            tabIndex={0}
            id="method-panel"
            aria-labelledby={`method-${method}`}
            className="method-panel"
            key={method}
          >
            <h3>{t(`controls.${methods[method]}.title`)}</h3>
            <p>{t(`controls.${methods[method]}.description`)}</p>
            <span>{t(`controls.${methods[method]}.detail`)}</span>
          </div>
          <button
            className="link-button"
            onClick={() => setOpen(!open)}
            aria-expanded={open}
            aria-controls="shortcut-sheet"
          >
            {t(open ? "shortcuts.hide" : "shortcuts.show")}
            <Icon
              name={open ? "plus" : "chevron"}
              className={open ? "rotate" : ""}
            />
          </button>
        </div>
      </div>
      <div className="shortcut-sheet" id="shortcut-sheet" hidden={!open}>
        {shortcutGroups.map((group) => (
          <div key={group.id}>
            <h3>{t(`shortcuts.${group.id}`)}</h3>
            {group.rows.map(([label, keys]) => (
              <div className="shortcut-row" key={label}>
                <span>{t(label)}</span>
                <kbd>{keys}</kbd>
              </div>
            ))}
          </div>
        ))}
        <p>{t("shortcuts.note")}</p>
      </div>
    </section>
  );
}

function NativeTiling() {
  const { t } = useLocale();
  const [layout, setLayout] = useState<"left" | "fill" | "right">("left");
  return (
    <section className="native-section section-pad" data-scroll-scene>
      <div className="native-header" data-reveal>
        <p className="eyebrow">{t("tiling.kicker")}</p>
        <h2>
          {t("tiling.title")}
          <br />
          <span>{t("tiling.accent")}</span>
        </h2>
        <p>{t("tiling.description")}</p>
      </div>
      <div className="tiling-visual" data-reveal>
        <div className={`tiling-screen layout-${layout}`} aria-hidden="true">
          <Wallpaper />
          <div className="tiling-menu">
            ●　 Finder　 {t("demo.menu")} <span>W　 ◒　 10:24</span>
          </div>
          <MacWindow index={3} className="tiling-back" />
          <MacWindow index={2} className="tiling-front" />
        </div>
        <div
          className="layout-picker"
          role="group"
          aria-label={t("tiling.label")}
        >
          {(["left", "fill", "right"] as const).map((item) => (
            <button
              key={item}
              aria-label={t("tiling.switch", { layout: t(`tiling.${item}`) })}
              aria-pressed={layout === item}
              onClick={() => setLayout(item)}
            >
              <span className={`layout-icon icon-${item}`} aria-hidden="true" />
              {t(`tiling.${item}`)}
            </button>
          ))}
        </div>
      </div>
      <p className="native-note">{t("tiling.note")}</p>
    </section>
  );
}

function Install() {
  const { t, formatNumber } = useLocale();
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
      <div className="install-heading" data-reveal>
        <img
          src={asset("whalu-icon.png")}
          width="96"
          height="96"
          alt={t("install.icon")}
        />
        <p className="eyebrow">{t("hero.kicker")}</p>
        <h2>
          {t("install.title")}
          <br />
          {t("install.titleEnd")}
        </h2>
        <p>{t("install.subtitle")}</p>
        <span className="compatibility">
          {t("install.compatibility", { version: "0.5.1" })}
        </span>
      </div>
      <div className="install-guide" data-reveal>
        <div>
          <span className="step-number">{formatNumber(1)}</span>
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
          <span className="step-number">{formatNumber(2)}</span>
          <h3>{t("install.permissions")}</h3>
          <p>{t("install.permissionsText")}</p>
        </div>
        <div>
          <span className="step-number">{formatNumber(3)}</span>
          <h3>{t("install.start")}</h3>
          <p>
            {t("install.hold")} <kbd>⌃</kbd> + <kbd>⌥</kbd>
            {t("install.startEnd")}
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

function Faq() {
  const { t } = useLocale();
  const questions = ["geometry", "privacy", "displays", "customize"] as const;

  return (
    <section className="faq-section section-pad">
      <h2 data-reveal>
        {t("faq.title")}
        <br />
        {t("faq.titleEnd")}
      </h2>
      <div className="faq-list" data-reveal>
        {questions.map((id) => (
          <details key={id}>
            <summary>
              {t(`faq.${id}.question`)}
              <span>
                <Icon name="plus" />
              </span>
            </summary>
            <p>{t(`faq.${id}.answer`)}</p>
          </details>
        ))}
      </div>
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
  const [open, setOpen] = useState(false);
  const header = useRef<HTMLElement>(null);
  const toggle = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    const closeOutside = (event: globalThis.PointerEvent) => {
      if (!header.current?.contains(event.target as Node)) setOpen(false);
    };
    const closeOnEscape = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
        toggle.current?.focus();
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [open]);
  return (
    <header className="product-nav" ref={header}>
      <div>
        <a
          className="brand"
          href="#"
          aria-label={t("nav.home")}
          onClick={() => setOpen(false)}
        >
          <img src={asset("whalu-icon.png")} width="28" height="28" alt="" />
          <span>Whalu</span>
        </a>
        <div className="nav-actions">
          <button
            className="nav-toggle"
            ref={toggle}
            aria-label={t("nav.explore")}
            aria-expanded={open}
            aria-controls="product-links"
            onClick={() => setOpen(!open)}
          >
            <Icon name="chevron" />
          </button>
          <nav
            id="product-links"
            className="nav-links"
            data-open={open}
            aria-label={t("nav.label")}
            onClick={() => setOpen(false)}
          >
            <a href="#overview">{t("nav.overview")}</a>
            <a href="#controls">{t("nav.controls")}</a>
            <a href="#get-whalu" className="nav-install">
              {t("nav.installation")}
            </a>
            {repository && (
              <a href={repository} target="_blank" rel="noreferrer">
                GitHub
              </a>
            )}
          </nav>
          <LanguageSelector />
          <a
            className="button button-small"
            href="#get-whalu"
            onClick={() => setOpen(false)}
          >
            {t("nav.get")}
          </a>
        </div>
      </div>
    </header>
  );
}

export function App() {
  const { t, locale } = useLocale();
  const { reducedMotion, systemReduced, toggleMotion } = useMotionPreference();
  useScrollMotion(reducedMotion, locale);
  return (
    <>
      <a className="skip-link" href="#main">
        {t("skip")}
      </a>
      <ProductNav />
      <main id="main" tabIndex={-1}>
        <section
          className="hero"
          data-scroll-scene
          aria-labelledby="hero-heading"
        >
          <div className="hero-copy">
            <p className="hero-kicker">{t("hero.kicker")}</p>
            <h1 id="hero-heading">
              {t("hero.title")}
              <br />
              <span>{t("hero.titleAccent")}</span>
            </h1>
            <p className="hero-description">
              {t("hero.description")}
              <br className="mobile-break" /> {t("hero.descriptionEnd")}
            </p>
            <div className="hero-actions">
              <a className="button" href="#get-whalu">
                {t("nav.get")}
              </a>
              <a className="text-link" href="#overview">
                {t("hero.learn")} <Icon name="chevron" />
              </a>
            </div>
            <p className="hero-availability">{t("hero.availability")}</p>
          </div>
          <ProductDemo />
        </section>
        <ScrollStory />
        <Controls />
        <NativeTiling />
        <section className="privacy-section section-pad">
          <div data-reveal>
            <Icon name="shield" />
            <h2>
              {t("privacy.title")}
              <br />
              <span>{t("privacy.accent")}</span>
            </h2>
            <p>
              {t("privacy.description")}
              <br className="desktop-break" /> {t("privacy.descriptionEnd")}
            </p>
          </div>
        </section>
        <Install />
        <Faq />
      </main>
      <footer className="site-footer">
        <div>
          <p>{t("footer.description")}</p>
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
