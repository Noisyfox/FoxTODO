import type { ReactNode, SyntheticEvent } from "react";
import {
  DndContext,
  MouseSensor,
  TouchSensor,
  KeyboardSensor,
  closestCenter,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
// The row owns navigation and sorting. Embedded controls keep their native gestures.
function isRowSurface(event: SyntheticEvent<HTMLDivElement>) {
  const control =
    event.target instanceof Element &&
    event.target.closest(
      'button, input, textarea, select, a[href], label, [contenteditable]:not([contenteditable="false"]), [role="button"], [role="checkbox"]',
    );
  return !control || control === event.currentTarget;
}

export function SortArea({
  ids,
  onMove,
  children,
  disabled = false,
}: {
  ids: string[];
  onMove: (id: string, target: string) => void;
  children: ReactNode;
  disabled?: boolean;
}) {
  const sensors = useSensors(
    useSensor(MouseSensor, { activationConstraint: { distance: 5 } }),
    useSensor(TouchSensor, {
      activationConstraint: { delay: 240, tolerance: 8 },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );
  return (
    <DndContext
      sensors={sensors}
      accessibility={{
        screenReaderInstructions: {
          draggable: "按空格开始排序，上下方向键移动，空格确认，Esc 取消。",
        },
      }}
      collisionDetection={closestCenter}
      onDragEnd={({ active, over }) => {
        if (!disabled && over && active.id !== over.id)
          onMove(String(active.id), String(over.id));
      }}
    >
      <SortableContext
        items={ids}
        strategy={verticalListSortingStrategy}
        disabled={disabled}
      >
        {children}
      </SortableContext>
    </DndContext>
  );
}

export function SortRow({
  id,
  label,
  className,
  children,
  onOpen,
  role = "group",
}: {
  id: string;
  label: string;
  className: string;
  children: ReactNode;
  onOpen?: () => void;
  role?: "group" | "button";
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id, attributes: { role } });
  return (
    <div
      ref={setNodeRef}
      {...(listeners ? attributes : {})}
      role={role}
      tabIndex={listeners || onOpen ? 0 : undefined}
      aria-label={label}
      aria-keyshortcuts={onOpen ? "Enter" : undefined}
      className={[
        className,
        listeners && "sortable-row",
        isDragging && "dragging",
      ]
        .filter(Boolean)
        .join(" ")}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      onMouseDown={(e) => {
        if (e.button === 0 && isRowSurface(e)) listeners?.onMouseDown?.(e);
      }}
      onTouchStart={(e) => {
        if (isRowSurface(e)) listeners?.onTouchStart?.(e);
      }}
      onKeyDown={(e) => {
        if (e.target !== e.currentTarget || isDragging) return;
        if (
          onOpen &&
          (e.key === "Enter" || (!listeners && e.code === "Space"))
        ) {
          e.preventDefault();
          onOpen();
        } else {
          listeners?.onKeyDown?.(e);
        }
      }}
      onClick={(e) => {
        if (isRowSurface(e)) onOpen?.();
      }}
    >
      {children}
    </div>
  );
}
