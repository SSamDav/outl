import { For, Show, createMemo, createSignal } from "solid-js";

interface CalendarProps {
  open: boolean;
  /** Slug of currently displayed journal (`YYYY-MM-DD`), or `null` for
   *  a non-journal page. The matching day is highlighted as
   *  "selected" in the grid. */
  selectedSlug: string | null;
  /** Today's slug, resolved by the parent so the header and calendar
   *  can't disagree on what "today" means (midnight rollover). */
  todaySlug: string | null;
  onClose: () => void;
  /** User picked a date. `slug` is `YYYY-MM-DD`. The parent is
   *  responsible for opening the journal (creating it on demand if
   *  the day has no entry yet). */
  onPick: (slug: string) => void;
}

const DAY_LABELS = ["S", "M", "T", "W", "T", "F", "S"];
const MONTH_NAMES = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

function parseSlug(slug: string | null): { year: number; month: number } | null {
  if (!slug) return null;
  const m = slug.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!m) return null;
  const year = Number(m[1]);
  const month = Number(m[2]) - 1;
  if (Number.isNaN(year) || Number.isNaN(month)) return null;
  return { year, month };
}

function formatSlug(year: number, month: number, day: number): string {
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${year}-${pad(month + 1)}-${pad(day)}`;
}

/**
 * Bottom-sheet mini-calendar. Navigates month-by-month and emits a
 * `YYYY-MM-DD` slug on tap. Today and the currently-viewed day are
 * highlighted distinctly; tapping the month/year label snaps the
 * grid back to today's month for quick orientation.
 *
 * Design intent mirrors iOS Calendar / Day One: chevron-based nav,
 * rounded "pill" day cells, accent fill for "selected" and accent
 * text for "today". Drag-to-dismiss handle matches the existing
 * `PageSwitcher` sheet so the two sheets feel like one family.
 */
export function Calendar(props: CalendarProps) {
  const initial = parseSlug(props.selectedSlug) ?? parseSlug(props.todaySlug);
  const [year, setYear] = createSignal(
    initial?.year ?? new Date().getFullYear(),
  );
  const [month, setMonth] = createSignal(
    initial?.month ?? new Date().getMonth(),
  );

  // When the sheet reopens (e.g. user navigated via header chevron and
  // tapped the calendar icon again), realign the visible month to the
  // current view's context so the user lands where they expect.
  let lastOpen = props.open;
  createMemo(() => {
    if (props.open && !lastOpen) {
      const next = parseSlug(props.selectedSlug) ?? parseSlug(props.todaySlug);
      if (next) {
        setYear(next.year);
        setMonth(next.month);
      }
    }
    lastOpen = props.open;
  });

  const days = createMemo(() => {
    const firstDay = new Date(year(), month(), 1);
    const firstWeekday = firstDay.getDay();
    const daysInMonth = new Date(year(), month() + 1, 0).getDate();
    const cells: Array<{ day: number; slug: string } | null> = [];
    for (let i = 0; i < firstWeekday; i += 1) cells.push(null);
    for (let d = 1; d <= daysInMonth; d += 1) {
      cells.push({ day: d, slug: formatSlug(year(), month(), d) });
    }
    // Pad to whole weeks so the grid stays rectangular.
    while (cells.length % 7 !== 0) cells.push(null);
    return cells;
  });

  function prevMonth() {
    if (month() === 0) {
      setYear((y) => y - 1);
      setMonth(11);
    } else {
      setMonth((m) => m - 1);
    }
  }

  function nextMonth() {
    if (month() === 11) {
      setYear((y) => y + 1);
      setMonth(0);
    } else {
      setMonth((m) => m + 1);
    }
  }

  function jumpToTodayMonth() {
    const now = new Date();
    setYear(now.getFullYear());
    setMonth(now.getMonth());
  }

  // Sheet drag-to-dismiss state. Same gesture pattern as PageSwitcher.
  const [dragY, setDragY] = createSignal(0);
  let dragStartY = 0;
  let dragActive = false;
  function onHandleDown(e: PointerEvent) {
    dragStartY = e.clientY;
    dragActive = true;
    (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
  }
  function onHandleMove(e: PointerEvent) {
    if (!dragActive) return;
    setDragY(Math.max(0, e.clientY - dragStartY));
  }
  function onHandleUp() {
    if (!dragActive) return;
    dragActive = false;
    if (dragY() > 80) {
      setDragY(0);
      props.onClose();
    } else {
      setDragY(0);
    }
  }

  return (
    <Show when={props.open}>
      <div
        class="fixed inset-0 z-50 bg-black/40 backdrop-blur-md outl-fade-in"
        onClick={props.onClose}
      />
      <div
        class="fixed inset-x-0 bottom-0 z-50 flex flex-col overflow-hidden rounded-t-2xl bg-(--color-ios-bg)/85 shadow-2xl outl-sheet-up backdrop-blur-2xl backdrop-saturate-150 dark:bg-(--color-iosd-bg)/85"
        style={{
          "padding-bottom": "env(safe-area-inset-bottom)",
          transform: `translateY(${dragY()}px)`,
          transition: dragActive
            ? "none"
            : "transform 200ms cubic-bezier(0.32, 0.72, 0, 1)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <header class="flex items-center gap-3 px-4 py-3">
          <span
            class="mx-auto block h-3 w-16 cursor-grab py-1 active:cursor-grabbing"
            style={{ "touch-action": "none" }}
            onPointerDown={onHandleDown}
            onPointerMove={onHandleMove}
            onPointerUp={onHandleUp}
            onPointerCancel={onHandleUp}
            aria-label="Drag to close"
            role="button"
          >
            <span
              aria-hidden="true"
              class="block h-1 w-10 mx-auto rounded-full bg-(--color-ios-divider) dark:bg-(--color-iosd-divider)"
            />
          </span>
        </header>

        <div class="flex items-center justify-between px-5 pb-3">
          <button
            type="button"
            aria-label="Previous month"
            onClick={prevMonth}
            class="rounded-full p-2 text-(--color-ios-accent) active:opacity-50 dark:text-(--color-iosd-accent)"
          >
            <svg
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path d="M15 18l-6-6 6-6" />
            </svg>
          </button>
          <button
            type="button"
            onClick={jumpToTodayMonth}
            class="text-[17px] font-semibold tabular-nums active:opacity-60"
          >
            {MONTH_NAMES[month()]} {year()}
          </button>
          <button
            type="button"
            aria-label="Next month"
            onClick={nextMonth}
            class="rounded-full p-2 text-(--color-ios-accent) active:opacity-50 dark:text-(--color-iosd-accent)"
          >
            <svg
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path d="M9 18l6-6-6-6" />
            </svg>
          </button>
        </div>

        <div class="grid grid-cols-7 px-3 pb-1 text-center text-[11px] font-medium uppercase tracking-wider text-(--color-ios-text-secondary) dark:text-(--color-iosd-text-secondary)">
          <For each={DAY_LABELS}>{(d) => <div>{d}</div>}</For>
        </div>

        <div class="grid grid-cols-7 gap-1 px-3 pb-5">
          <For each={days()}>
            {(cell) => {
              if (!cell) return <div class="aspect-square" />;
              const isToday = cell.slug === props.todaySlug;
              const isSelected = cell.slug === props.selectedSlug;
              return (
                <button
                  type="button"
                  onClick={() => props.onPick(cell.slug)}
                  class="flex aspect-square items-center justify-center rounded-full text-[15px] tabular-nums active:opacity-50"
                  classList={{
                    "bg-(--color-ios-accent) text-white font-semibold dark:bg-(--color-iosd-accent)":
                      isSelected,
                    "text-(--color-ios-accent) font-semibold dark:text-(--color-iosd-accent)":
                      isToday && !isSelected,
                  }}
                >
                  {cell.day}
                </button>
              );
            }}
          </For>
        </div>
      </div>
    </Show>
  );
}
