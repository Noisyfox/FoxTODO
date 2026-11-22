import type { ReactNode } from "react";
import {
  DndContext,
  PointerSensor,
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
import { GripVertical } from "lucide-react";

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
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );
  return (
    <DndContext
      sensors={sensors}
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
}: {
  id: string;
  label: string;
  className: string;
  children: ReactNode;
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id });
  return (
    <div
      ref={setNodeRef}
      className={className + (isDragging ? " dragging" : "")}
      style={{ transform: CSS.Transform.toString(transform), transition }}
    >
      {!attributes["aria-disabled"] && (
        <button
          type="button"
          className="drag-handle"
          ref={setActivatorNodeRef}
          {...attributes}
          {...listeners}
          aria-label={"拖动排序：" + label}
          title="拖动排序"
        >
          <GripVertical size={15} />
        </button>
      )}
      {children}
    </div>
  );
}
