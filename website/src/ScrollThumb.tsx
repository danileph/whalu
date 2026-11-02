import { useLayoutEffect, useRef } from "react";
import type { KeyboardEvent, PointerEvent } from "react";
import { useLocale } from "./i18n/I18nProvider";

export function ScrollThumb() {
  const { t } = useLocale();
  const thumb = useRef<HTMLDivElement>(null);
  const geometry = useRef({ maxScroll: 0, travel: 0 });
  const drag = useRef<{ pointer: number; y: number; scroll: number } | null>(
    null,
  );

  useLayoutEffect(() => {
    const node = thumb.current!;
    const root = document.documentElement;
    root.dataset.scrollbar = "custom";
    let frame = 0;
    function update() {
      frame = 0;
      const height = root.clientHeight;
      const maxScroll = Math.max(0, root.scrollHeight - height);
      const size = Math.min(
        height - 8,
        Math.max(32, (height * height) / root.scrollHeight),
      );
      const travel = Math.max(0, height - size - 8);
      const progress = maxScroll
        ? Math.max(0, Math.min(1, window.scrollY / maxScroll))
        : 0;
      geometry.current = { maxScroll, travel };
      node.hidden = !maxScroll;
      node.style.height = `${size}px`;
      node.style.transform = `translateY(${4 + progress * travel}px)`;
      node.setAttribute("aria-valuenow", String(Math.round(progress * 100)));
    }
    function schedule() {
      if (!frame) frame = requestAnimationFrame(update);
    }
    const observer = new ResizeObserver(schedule);
    observer.observe(document.body);
    window.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    update();
    return () => {
      observer.disconnect();
      cancelAnimationFrame(frame);
      window.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
      delete root.dataset.scrollbar;
    };
  }, []);

  function keyDown(event: KeyboardEvent<HTMLDivElement>) {
    const steps: Record<string, number> = {
      ArrowUp: -40,
      ArrowDown: 40,
      PageUp: -window.innerHeight * 0.9,
      PageDown: window.innerHeight * 0.9,
      Home: -window.scrollY,
      End: geometry.current.maxScroll - window.scrollY,
    };
    if (!(event.key in steps)) return;
    event.preventDefault();
    window.scrollBy({ top: steps[event.key], behavior: "instant" });
  }

  function release(event: PointerEvent<HTMLDivElement>) {
    drag.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId))
      event.currentTarget.releasePointerCapture(event.pointerId);
  }

  return (
    <div
      ref={thumb}
      className="scroll-thumb"
      role="scrollbar"
      aria-label={t("scrollbar.label")}
      aria-controls="main"
      aria-orientation="vertical"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={0}
      tabIndex={0}
      hidden
      onKeyDown={keyDown}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        event.currentTarget.focus({ preventScroll: true });
        event.currentTarget.setPointerCapture(event.pointerId);
        drag.current = {
          pointer: event.pointerId,
          y: event.clientY,
          scroll: window.scrollY,
        };
      }}
      onPointerMove={(event) => {
        const origin = drag.current;
        const { maxScroll, travel } = geometry.current;
        if (!origin || origin.pointer !== event.pointerId || !travel) return;
        window.scrollTo({
          top:
            origin.scroll + ((event.clientY - origin.y) * maxScroll) / travel,
          behavior: "instant",
        });
      }}
      onPointerUp={release}
      onPointerCancel={release}
      onLostPointerCapture={() => {
        drag.current = null;
      }}
    />
  );
}
