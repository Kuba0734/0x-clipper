import { Icon, type IconName } from "@/components/ui/Icon";
import { useT, type TKey } from "@/i18n";
import { useUi, type Page } from "@/stores/ui";

const items: { page: Page; key: TKey; icon: IconName }[] = [
  { page: "dashboard", key: "nav.dashboard", icon: "dashboard" },
  { page: "library", key: "nav.library", icon: "film" },
  { page: "settings", key: "nav.settings", icon: "settings" },
];

const ITEM_H = 40;
const GAP = 4;

/**
 * Navigation als Glas-Fläche. Die weiße Markierung gleitet per translateY; der Wechsel
 * selbst passiert sofort beim Klick (keine Verzögerung durch Animationen).
 */
export function Sidebar() {
  const t = useT();
  const { page, go } = useUi();
  const idx = items.findIndex((i) => i.page === page);
  return (
    <nav className="glass relative m-3 mr-0 flex w-52 shrink-0 flex-col gap-1 rounded-lg !border-border p-2">
      <span
        aria-hidden
        className="slide-indicator absolute top-2 right-2 left-2 rounded-md bg-accent"
        style={{ height: ITEM_H, transform: `translateY(${idx * (ITEM_H + GAP)}px)` }}
      />
      {items.map(({ page: p, key, icon }) => {
        const active = p === page;
        return (
          <button
            key={p}
            type="button"
            onClick={() => go(p)}
            aria-current={active ? "page" : undefined}
            style={{ height: ITEM_H }}
            className={`press relative flex items-center gap-3 rounded-md px-3 text-[14px] font-medium ${
              active ? "text-on-accent" : "hoverable text-muted hover:text-text"
            }`}
          >
            <Icon name={icon} />
            <span>{t(key)}</span>
          </button>
        );
      })}
    </nav>
  );
}
