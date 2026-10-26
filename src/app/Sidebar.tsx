import { m } from "motion/react";
import { Film, LayoutDashboard, Settings } from "lucide-react";

import { useT, type TKey } from "@/i18n";
import { spring } from "@/lib/motion";
import { useUi, type Page } from "@/stores/ui";

const items: { page: Page; key: TKey; icon: typeof Film }[] = [
  { page: "dashboard", key: "nav.dashboard", icon: LayoutDashboard },
  { page: "library", key: "nav.library", icon: Film },
  { page: "settings", key: "nav.settings", icon: Settings },
];

/** Navigation; die aktive Markierung gleitet per layoutId (Akzentverlauf). */
export function Sidebar() {
  const t = useT();
  const { page, go } = useUi();
  return (
    <nav className="flex w-56 shrink-0 flex-col gap-1 border-r border-border/60 p-3">
      {items.map(({ page: p, key, icon: Icon }) => {
        const active = p === page;
        return (
          <button
            key={p}
            type="button"
            onClick={() => go(p)}
            aria-current={active ? "page" : undefined}
            className={`relative flex h-10 items-center gap-3 rounded-[12px] px-3 text-[14px] font-medium transition-colors ${
              active ? "text-white" : "text-muted hover:bg-surface hover:text-text"
            }`}
          >
            {active && (
              <m.span layoutId="nav-active" transition={spring} className="absolute inset-0 rounded-[12px] bg-gradient-accent" />
            )}
            <Icon className="relative size-[18px]" />
            <span className="relative">{t(key)}</span>
          </button>
        );
      })}
    </nav>
  );
}
