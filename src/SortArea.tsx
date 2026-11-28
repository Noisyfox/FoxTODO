import {
  createContext,
  useContext,
  useId,
  type ReactNode,
  type SyntheticEvent,
} from "react";
import {
  DndContext,
  DragOverlay,
  pointerWithin,
  useDndContext,
  useDroppable,
  type Active,
  type CollisionDetection,
  type DroppableContainer,
  type KeyboardCoordinateGetter,
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
import { CSS, getEventCoordinates } from "@dnd-kit/utilities";
// The row owns navigation and sorting. Embedded controls keep their native gestures.
function isRowSurface(event: SyntheticEvent<HTMLDivElement>) {
  const control =
    event.target instanceof Element &&
    event.target.closest(
      'button, input, textarea, select, a[href], label, [contenteditable]:not([contenteditable="false"]), [role="button"], [role="checkbox"]',
    );
  return !control || control === event.currentTarget;
}

const MoveWithinGroup = createContext<
  ((id: string, target: string) => void) | undefined
>(undefined);

function accepts(active: Active, target: DroppableContainer) {
  if (target.id === active.id) return true;
  const drag = active.data.current;
  const drop = target.data.current;
  if (drop?.listTarget)
    return !!drag?.task && drag.task.list !== drop.listTarget;
  return (
    !!drag?.onMove && drag.sortable.containerId === drop?.sortable?.containerId
  );
}

const collisions: CollisionDetection = (args) => {
  const containers = args.droppableContainers.filter((c) =>
    accepts(args.active, c),
  );
  if (args.pointerCoordinates && args.active.data.current?.task) {
    const { x, y } = args.pointerCoordinates;
    const underPointer = document.elementFromPoint(x, y);
    const list = containers.find(
      (c) =>
        c.data.current?.listTarget && c.node.current?.contains(underPointer),
    );
    if (list) return [{ id: list.id }];
    // Blank space, the current list and smart views are not drop targets.
    if (!underPointer?.closest(".task-list")) return [];
    return pointerWithin({
      ...args,
      droppableContainers: containers.filter(
        (c) => !c.data.current?.listTarget,
      ),
    });
  }
  if (!args.pointerCoordinates && args.active.data.current?.task) {
    // The keyboard sensor aligns top-left corners even across different row widths.
    const { left, top } = args.collisionRect;
    return pointerWithin({
      ...args,
      pointerCoordinates: { x: left + 1, y: top + 1 },
      droppableContainers: containers,
    });
  }
  return closestCenter({ ...args, droppableContainers: containers });
};

const keyboardCoordinates: KeyboardCoordinateGetter = (event, args) => {
  const { context } = args;
  const rects = new Map(context.droppableRects);
  for (const c of context.droppableContainers.getEnabled()) {
    if (
      !context.active ||
      !accepts(context.active, c) ||
      !rects.get(c.id)?.width
    )
      rects.delete(c.id);
  }
  return sortableKeyboardCoordinates(event, {
    ...args,
    context: { ...context, droppableRects: rects },
  });
};

export function WorkspaceDrag({
  children,
  onTransfer,
}: {
  children: ReactNode;
  onTransfer: (source: string, destination: string, task: string) => void;
}) {
  const sensors = useSensors(
    useSensor(MouseSensor, { activationConstraint: { distance: 5 } }),
    useSensor(TouchSensor, {
      activationConstraint: { delay: 240, tolerance: 8 },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: keyboardCoordinates,
    }),
  );
  return (
    <DndContext
      sensors={sensors}
      accessibility={{
        screenReaderInstructions: {
          draggable:
            "按空格开始拖动，用方向键选择位置或目标清单，空格确认，Esc 取消。",
        },
      }}
      collisionDetection={collisions}
      onDragEnd={({ active, over }) => {
        if (!over || active.id === over.id) return;
        const drag = active.data.current;
        const destination = over.data.current?.listTarget;
        if (drag?.task && destination) {
          onTransfer(drag.task.list, destination, drag.task.id);
        } else {
          drag?.onMove?.(String(active.id), String(over.id));
        }
      }}
    >
      {children}
      <TaskPreview />
    </DndContext>
  );
}

function TaskPreview() {
  const { active, activatorEvent, activeNodeRect } = useDndContext();
  const data = active?.data.current;
  const pointer = activatorEvent && getEventCoordinates(activatorEvent);
  return (
    <DragOverlay dropAnimation={null} className="pointer-events-none">
      {data?.task && (
        <div
          aria-hidden="true"
          className="w-fit max-w-48 truncate rounded-md border bg-card px-3 py-1.5 text-sm shadow-md"
          style={
            pointer && activeNodeRect
              ? {
                  marginLeft: pointer.x - activeNodeRect.left + 8,
                  marginTop: pointer.y - activeNodeRect.top - 38,
                }
              : undefined
          }
        >
          {data.label}
        </div>
      )}
    </DragOverlay>
  );
}

export function ListTarget({
  list,
  children,
}: {
  list: string;
  children: ReactNode;
}) {
  const id = useId();
  const { setNodeRef, isOver } = useDroppable({
    id,
    data: { listTarget: list },
  });
  return (
    <div ref={setNodeRef} className={isOver ? "list-drop-target" : undefined}>
      {children}
    </div>
  );
}

export function MobileListTargets({
  lists,
}: {
  lists: { id: string; name: string; emoji?: string; group?: string }[];
}) {
  const { active, activatorEvent } = useDndContext();
  const pointer = activatorEvent && getEventCoordinates(activatorEvent);
  const source = active?.data.current?.task?.list;
  const targets = lists.filter((list) => list.id !== source);
  if (!source || !targets.length) return null;
  return (
    <section
      className="move-targets"
      data-side={
        pointer && pointer.y > window.innerHeight / 2 ? "top" : "bottom"
      }
    >
      <h3 className="mb-1 text-xs font-semibold text-muted-foreground">
        移动到清单
      </h3>
      <div className="min-h-0 overflow-y-auto">
        {targets.map((list) => (
          <ListTarget key={list.id} list={list.id}>
            <div className="sidebar-list-row">
              <span className="truncate">{list.emoji} {list.name}</span>
              {list.group && <small className="ml-auto truncate text-muted-foreground">{list.group}</small>}
            </div>
          </ListTarget>
        ))}
      </div>
    </section>
  );
}

export function SortArea({
  ids,
  onMove,
  children,
  disabled = false,
}: {
  ids: string[];
  onMove?: (id: string, target: string) => void;
  children: ReactNode;
  disabled?: boolean;
}) {
  return (
    <SortableContext
      items={ids}
      strategy={verticalListSortingStrategy}
      disabled={disabled}
    >
      <MoveWithinGroup.Provider value={onMove}>
        {children}
      </MoveWithinGroup.Provider>
    </SortableContext>
  );
}

export function SortRow({
  id,
  label,
  className,
  children,
  onOpen,
  role = "group",
  task,
}: {
  id: string;
  label: string;
  className: string;
  children: ReactNode;
  onOpen?: () => void;
  role?: "group" | "button";
  task?: { list: string; id: string };
}) {
  const onMove = useContext(MoveWithinGroup);
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id, attributes: { role }, data: { task, label, onMove } });
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
