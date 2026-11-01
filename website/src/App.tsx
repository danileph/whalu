import { useEffect, useId, useRef, useState } from "react";
import type { CSSProperties, KeyboardEvent, ReactNode, SVGProps } from "react";

const repository = (
  import.meta.env.VITE_REPOSITORY_URL as string | undefined
)?.replace(/\/$/, "");
const asset = (name: string) => `${import.meta.env.BASE_URL}${name}`;
const clamp = (value: number) => Math.max(0, Math.min(1, value));

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
    | "shield";
}) {
  const paths: Record<string, ReactNode> = {
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
function useScrollMotion() {
  useEffect(() => {
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
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
            if (!media.matches && typeof entry.target.animate === "function") {
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
        const progress = media.matches
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
    const onMotionChange = () => {
      if (media.matches)
        revealAnimations.forEach((animation) => animation.finish());
      schedule();
    };
    update();
    window.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    media.addEventListener("change", onMotionChange);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      revealAnimations.forEach((animation) => animation.cancel());
      window.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
      media.removeEventListener("change", onMotionChange);
    };
  }, []);
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
    name: "Finder",
    title: "Projects",
    glyph: "⌑",
    color: "#2589ed",
  },
  {
    id: "editor",
    name: "Editor",
    title: "Whalu — main.rs",
    glyph: "⌘",
    color: "#4669df",
  },
  {
    id: "safari",
    name: "Safari",
    title: "Project Atlas",
    glyph: "↗",
    color: "#007aff",
  },
  {
    id: "notes",
    name: "Notes",
    title: "What’s next",
    glyph: "≡",
    color: "#e6ad25",
  },
  {
    id: "terminal",
    name: "Terminal",
    title: "whalu — zsh",
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
  if (id === "safari")
    return (
      <div className="safari-body">
        <aside>
          <span className="atlas-logo">a.</span>
          <span>Workspace</span>
          <b>◈ Overview</b>
          <span>▦ Projects</span>
          <span>◷ Timeline</span>
          <span>☷ Resources</span>
          <div className="avatar-row">
            <i>J</i>
            <i>A</i>
            <i>M</i>
            <small>Design team</small>
          </div>
        </aside>
        <div className="atlas-main">
          <div className="atlas-top">
            <span>Workspace / Projects</span>
            <span>◉　⋯</span>
          </div>
          <div className="atlas-heading">
            <div>
              <small>YOUR NEXT BIG THING</small>
              <h4>Project Atlas</h4>
              <p>A fresh perspective on what’s possible.</p>
            </div>
            <span className="project-tag">In progress</span>
          </div>
          <div className="atlas-art">
            <div className="chrome-orbit orbit-one" />
            <div className="chrome-orbit orbit-two" />
            <div className="chrome-orbit orbit-three" />
            <span>Room for a new perspective.</span>
            <small>ATLAS® / DESIGN EXPLORATIONS</small>
          </div>
          <div className="atlas-tasks">
            <span>
              <i className="task-dot blue" /> Research <b>Complete</b>
            </span>
            <span>
              <i className="task-dot purple" /> Design <b>In progress</b>
            </span>
            <span>
              <i className="task-dot green" /> Development <b>Up next</b>
            </span>
          </div>
        </div>
      </div>
    );
  if (id === "notes")
    return (
      <div className="notes-body">
        <aside>
          <span>Notes</span>
          <b>
            All Notes <i>12</i>
          </b>
          <span>
            Projects <i>5</i>
          </span>
          <span>
            Personal <i>7</i>
          </span>
        </aside>
        <div>
          <time>November 1, 2026 at 10:24 AM</time>
          <h4>What’s next.</h4>
          <p>A few things to move forward.</p>
          <ul>
            <li>
              <i className="checked">✓</i> A fresh start
            </li>
            <li>
              <i className="checked">✓</i> Space for the next idea
            </li>
            <li>
              <i /> Build something worth using
            </li>
          </ul>
          <div className="note-rule" />
          <p className="note-quiet">One window at a time.</p>
        </div>
      </div>
    );
  if (id === "finder")
    return (
      <div className="finder-body">
        <aside>
          <small>Favorites</small>
          <span>◷ Recents</span>
          <span>⌂ Desktop</span>
          <b>▱ Projects</b>
          <span>↓ Downloads</span>
          <span>☁ iCloud Drive</span>
        </aside>
        <div className="folders">
          {[
            "Whalu",
            "Design",
            "Photography",
            "Weekend",
            "Archive",
            "Resources",
          ].map((name) => (
            <div key={name}>
              <i className="folder" />
              <span>{name}</span>
            </div>
          ))}
        </div>
      </div>
    );
  if (id === "editor")
    return (
      <div className="editor-body">
        <aside>
          <small>EXPLORER</small>
          <span>⌄ WHALU</span>
          <span>　⌄ src</span>
          <b>　　main.rs</b>
          <span>　　navigation.rs</span>
          <span>　Cargo.toml</span>
        </aside>
        <div className="code-pane">
          <span className="code-tab">main.rs　×</span>
          <pre>
            <span className="code-comment">// Every window has its place.</span>
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
          <span>{apps[index].title}</span>
        )}
        <span className="window-tools">⊞</span>
      </div>
      <WindowBody id={apps[index].id} />
    </div>
  );
}

function ProductDemo() {
  const [selected, setSelected] = useState(2);
  const wheelTime = useRef(0);
  const select = (index: number) =>
    setSelected(Math.max(0, Math.min(apps.length - 1, index)));
  function keyDown(event: KeyboardEvent<HTMLDivElement>) {
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
            aria-label="Interactive window overview. Use the arrow keys or select a window."
            onKeyDown={keyDown}
          >
            <Wallpaper />
            <div className="menu-bar" aria-hidden="true">
              <span>
                ●　 <b>{apps[selected].name}</b>　 File　 Edit　 View　 Window
              </span>
              <span>W　 ◒　 Sun 10:24 AM</span>
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
                    aria-label={`Switch to ${app.name}`}
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
        <span>Take it for a spin.</span>
        <div className="demo-apps" aria-label="Choose a demo window">
          {apps.map((app, index) => (
            <button
              key={app.id}
              aria-label={`Show ${app.name}`}
              aria-pressed={selected === index}
              onClick={() => select(index)}
            >
              {app.name}
            </button>
          ))}
        </div>
        <span className="demo-selection" role="status">
          {apps[selected].name} <span>0{selected + 1} / 05</span>
        </span>
      </div>
      <p className="demo-disclaimer">
        Interactive illustration of the Whalu overview.
      </p>
    </div>
  );
}

const chapters = [
  {
    label: "See every window",
    title: (
      <>
        Every window.
        <br />
        <span>Its own place.</span>
      </>
    ),
    text: "Three browser windows. Two documents. Your terminal. Whalu gives each one a preview, so you can go straight to the one you need.",
  },
  {
    label: "Find your focus",
    title: (
      <>
        One movement.
        <br />
        <span>Right back to it.</span>
      </>
    ),
    text: "A three-finger swipe, a shortcut, or a scroll. Move through your windows while their previews glide into place. Your focus follows along.",
  },
  {
    label: "Keep your order",
    title: (
      <>
        Your windows.
        <br />
        <span>Your way.</span>
      </>
    ),
    text: "Drag a preview to change its place in the stack. Your windows stay exactly where you left them on the desktop.",
  },
];

function ScrollStory() {
  const ref = useRef<HTMLElement>(null);
  function goToChapter(index: number) {
    const section = ref.current;
    if (!section) return;
    const range = Math.max(1, section.offsetHeight - window.innerHeight);
    const reduced = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;
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
      aria-label="Explore the Whalu overview"
    >
      <div className="story-sticky">
        <div className="story-inner">
          <div className="story-copy">
            <p className="eyebrow">A new perspective.</p>
            <div className="story-chapters">
              {chapters.map((chapter, index) => (
                <div className={`chapter chapter-${index}`} key={chapter.label}>
                  <h2>{chapter.title}</h2>
                  <p>{chapter.text}</p>
                </div>
              ))}
            </div>
            <div className="story-scroll-cue">
              <span /> Scroll to explore
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
          <div className="story-navigation" aria-label="Overview chapters">
            {chapters.map((chapter, index) => (
              <button
                key={chapter.label}
                data-chapter-button
                aria-pressed={index === 0}
                onClick={() => goToChapter(index)}
              >
                <span>0{index + 1}</span>
                {chapter.label}
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
    title: "Navigate",
    rows: [
      ["Previous / next window", "⌃ ⌥ ↑ / ↓"],
      ["Another window of the same app", "⌃ ⌥ ← / →"],
      ["Next window in the stack", "⌘ Tab"],
    ],
  },
  {
    title: "Arrange",
    rows: [
      ["Reorder a window in the stack", "⌃ ⌥ ⌘ ↑ / ↓"],
      ["Move to an adjacent Space", "⌃ ⌥ ⌘ ← / →"],
      ["Tile to the left / right half", "⌃ ⌘ ← / →"],
    ],
  },
];

function Controls() {
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
  const methods = [
    {
      name: "Keyboard",
      icon: "keyboard" as const,
      title: "Stay in your flow.",
      description:
        "Hold Control + Option to see your windows. Navigate with the arrows or H, J, K, L. Even Command + Tab switches individual windows.",
      detail: "Familiar shortcuts. A more precise destination.",
    },
    {
      name: "Trackpad",
      icon: "trackpad" as const,
      title: "It comes naturally.",
      description:
        "Rest three fingers to reveal the overview. Swipe to move between windows. Lift your fingers to get back to work.",
      detail: "Native four-finger Space gestures stay available.",
    },
    {
      name: "Mouse",
      icon: "mouse" as const,
      title: "Point. Scroll. You’re there.",
      description:
        "Move to the left edge of your display to see your windows. Scroll through the previews, or click the one you want.",
      detail: "Back and Forward buttons navigate while the overview is open.",
    },
  ];
  return (
    <section className="controls-section section-pad" id="controls">
      <div className="section-heading" data-reveal>
        <p className="eyebrow">Made for your muscle memory.</p>
        <h2>
          Feels familiar.
          <br />
          <span>Works your way.</span>
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
          <div className="method-tabs" role="tablist" aria-label="Input method">
            {methods.map((item, index) => (
              <button
                key={item.name}
                role="tab"
                id={`method-${index}`}
                aria-controls="method-panel"
                aria-selected={method === index}
                tabIndex={method === index ? 0 : -1}
                onClick={() => setMethod(index)}
                onKeyDown={(event) => onTabKey(event, index)}
              >
                <Icon name={item.icon} />
                {item.name}
              </button>
            ))}
          </div>
          <div
            role="tabpanel"
            id="method-panel"
            aria-labelledby={`method-${method}`}
            className="method-panel"
            key={method}
          >
            <h3>{methods[method].title}</h3>
            <p>{methods[method].description}</p>
            <span>{methods[method].detail}</span>
          </div>
          <button
            className="link-button"
            onClick={() => setOpen(!open)}
            aria-expanded={open}
            aria-controls="shortcut-sheet"
          >
            {open ? "Hide keyboard shortcuts" : "See keyboard shortcuts"}
            <Icon
              name={open ? "plus" : "chevron"}
              className={open ? "rotate" : ""}
            />
          </button>
        </div>
      </div>
      <div className="shortcut-sheet" id="shortcut-sheet" hidden={!open}>
        {shortcutGroups.map((group) => (
          <div key={group.title}>
            <h3>{group.title}</h3>
            {group.rows.map(([label, keys]) => (
              <div className="shortcut-row" key={label}>
                <span>{label}</span>
                <kbd>{keys}</kbd>
              </div>
            ))}
          </div>
        ))}
        <p>
          Shortcuts are configurable. Keep Space and Mission Control gestures on
          four fingers, with three-finger dragging disabled.
        </p>
      </div>
    </section>
  );
}

function NativeTiling() {
  const [layout, setLayout] = useState<"left" | "fill" | "right">("left");
  return (
    <section className="native-section section-pad" data-scroll-scene>
      <div className="native-header" data-reveal>
        <p className="eyebrow">Right at home on your Mac.</p>
        <h2>
          A perfect fit.
          <br />
          <span>By macOS.</span>
        </h2>
        <p>
          Left, right, center, or all the room you need. Whalu uses native macOS
          tiling, so your windows fit just as they should.
        </p>
      </div>
      <div className="tiling-visual" data-reveal>
        <div className={`tiling-screen layout-${layout}`} aria-hidden="true">
          <Wallpaper />
          <div className="tiling-menu">
            ●　 Finder　 File　 Edit　 View <span>W　 ◒　 10:24</span>
          </div>
          <MacWindow index={3} className="tiling-back" />
          <MacWindow index={2} className="tiling-front" />
        </div>
        <div
          className="layout-picker"
          aria-label="Try native window arrangements"
        >
          {(["left", "fill", "right"] as const).map((item) => (
            <button
              key={item}
              aria-label={`Tile ${item}`}
              aria-pressed={layout === item}
              onClick={() => setLayout(item)}
            >
              <span className={`layout-icon icon-${item}`} aria-hidden="true" />
              {item === "fill" ? "Fill" : item === "left" ? "Left" : "Right"}
            </button>
          ))}
        </div>
      </div>
      <p className="native-note">
        Your windows stay put until you tell them otherwise.
      </p>
    </section>
  );
}

function Install() {
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
          alt="Whalu app icon"
        />
        <p className="eyebrow">Whalu for Mac</p>
        <h2>
          Make room
          <br />
          for what’s next.
        </h2>
        <p>Free. Open source. All yours.</p>
        <span className="compatibility">macOS 15 or later · v0.5.1</span>
      </div>
      <div className="install-guide" data-reveal>
        <div>
          <span className="step-number">1</span>
          <h3>Build and install.</h3>
          <p>
            {repository ? (
              <>
                Clone{" "}
                <a href={repository} target="_blank" rel="noreferrer">
                  the repository
                </a>{" "}
                and run this from its folder.
              </>
            ) : (
              "Run this from your local Whalu source folder."
            )}
          </p>
          <div className="install-command">
            <code>./scripts/install.sh</code>
            <button
              onClick={copy}
              aria-label={copied ? "Command copied" : "Copy install command"}
            >
              <Icon name={copied ? "check" : "copy"} />
            </button>
          </div>
          <small role="status">
            {copied
              ? "Copied to clipboard."
              : error
                ? "Select and copy the command above."
                : "Requires Rust and Apple’s Command Line Tools."}
          </small>
        </div>
        <div>
          <span className="step-number">2</span>
          <h3>Give Whalu access.</h3>
          <p>
            Enable Accessibility for window navigation. Allow Screen &amp;
            System Audio Recording for previews. Whalu guides you through setup.
          </p>
        </div>
        <div>
          <span className="step-number">3</span>
          <h3>Find your next window.</h3>
          <p>
            Hold <kbd>⌃</kbd> + <kbd>⌥</kbd>, swipe with three fingers, or visit
            the left edge of your screen. You’re all set.
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
          Explore the source <Icon name="chevron" />
        </a>
      )}
    </section>
  );
}

function Faq() {
  const questions = [
    [
      "Will Whalu rearrange my windows?",
      "Your windows stay where you put them. Whalu changes focus and the order of its window stack. Tiling happens only when you request it, using macOS’s own window commands.",
    ],
    [
      "Do window previews leave my Mac?",
      "No. Whalu takes small snapshots of individual windows while the overview is open and keeps them in memory. It doesn’t upload previews, save them to disk, or record audio. Without permission, the overview shows app icons instead.",
    ],
    [
      "What about multiple displays and Spaces?",
      "Each display keeps its own window order and remembered selection across Spaces. The overview appears on the display under your pointer and shows windows in its current Space. Shortcuts can also send the focused window to an adjacent desktop.",
    ],
    [
      "Can I customize the controls?",
      "Yes. Open the configuration from the W menu to change shortcuts, turn gestures on or off, or ignore specific apps. Reload the configuration from the same menu. Whalu is open source under the MIT license.",
    ],
  ];
  return (
    <section className="faq-section section-pad">
      <h2 data-reveal>
        A few more things
        <br />
        to know.
      </h2>
      <div className="faq-list" data-reveal>
        {questions.map(([question, answer]) => (
          <details key={question}>
            <summary>
              {question}
              <span>
                <Icon name="plus" />
              </span>
            </summary>
            <p>{answer}</p>
          </details>
        ))}
      </div>
    </section>
  );
}

export function App() {
  useScrollMotion();
  return (
    <>
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <header className="product-nav">
        <div>
          <a className="brand" href="#" aria-label="Whalu home">
            <img src={asset("whalu-icon.png")} width="28" height="28" alt="" />
            <span>Whalu</span>
          </a>
          <nav aria-label="Main navigation">
            <a href="#overview">Overview</a>
            <a href="#controls">Controls</a>
            {repository && (
              <a href={repository} target="_blank" rel="noreferrer">
                GitHub
              </a>
            )}
            <a className="button button-small" href="#get-whalu">
              Get Whalu
            </a>
          </nav>
        </div>
      </header>
      <main id="main">
        <section
          className="hero"
          data-scroll-scene
          aria-labelledby="hero-heading"
        >
          <div className="hero-copy">
            <p className="hero-kicker">Whalu for Mac</p>
            <h1 id="hero-heading">
              Every window.
              <br />
              <span>A swipe away.</span>
            </h1>
            <p className="hero-description">
              A whole new way to move
              <br className="mobile-break" /> through your Mac.
            </p>
            <div className="hero-actions">
              <a className="button" href="#get-whalu">
                Get Whalu
              </a>
              <a className="text-link" href="#overview">
                See how it works <Icon name="chevron" />
              </a>
            </div>
            <p className="hero-availability">
              Free and open source. macOS 15 or later.
            </p>
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
              Your windows.
              <br />
              <span>Your business.</span>
            </h2>
            <p>
              Window previews stay in memory on your Mac.
              <br className="desktop-break" /> No uploads. No account. No cloud.
            </p>
          </div>
        </section>
        <Install />
        <Faq />
      </main>
      <footer className="site-footer">
        <div>
          <p>
            Whalu is an independent, open-source project for macOS. The product
            demonstrations on this page are interactive illustrations.
          </p>
          <div className="footer-bottom">
            <a href="#" className="footer-brand">
              Whalu
            </a>
            <span>Open source. MIT licensed.</span>
            <a href={asset("THIRD_PARTY_NOTICES.txt")}>Third-party notices</a>
            <a href="#" className="back-top">
              Back to top ↑
            </a>
          </div>
        </div>
      </footer>
    </>
  );
}
