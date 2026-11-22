import { useId } from "react";
import { Button } from "@/components/ui/button";
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
  ComboboxTrigger,
} from "@/components/ui/combobox";

type Option = { value: string; label: string };

export function OptionCombobox({
  label,
  options,
  value,
  onValueChange,
  compact = false,
  container,
  id,
}: {
  label: string;
  options: Option[];
  value: string;
  onValueChange: (value: string) => void;
  compact?: boolean;
  container?: React.RefObject<HTMLElement | null>;
  id?: string;
}) {
  const valueId = useId();
  const selected = options.find((item) => item.value === value) ?? null;
  return (
    <Combobox
      items={options}
      value={selected}
      itemToStringLabel={(item) => item.label}
      itemToStringValue={(item) => item.value}
      autoHighlight
      onValueChange={(item) => {
        if (item && item.value !== value) onValueChange(item.value);
      }}
    >
      <ComboboxTrigger
        id={id}
        aria-label={label}
        aria-describedby={valueId}
        render={
          <Button
            variant={compact ? "ghost" : "outline"}
            size={compact ? "sm" : "default"}
          />
        }
        className={compact ? "justify-between" : "w-full justify-between"}
      >
        <span id={valueId} className="truncate">
          {selected?.label ?? label}
        </span>
      </ComboboxTrigger>
      <ComboboxContent container={container} align={compact ? "end" : "start"}>
        <div className="p-1">
          <ComboboxInput
            aria-label={`搜索${label}选项`}
            placeholder="搜索选项…"
            className="h-8 shadow-none"
          />
        </div>
        <ComboboxEmpty>没有匹配的选项</ComboboxEmpty>
        <ComboboxList>
          {(item: Option) => (
            <ComboboxItem key={item.value} value={item}>
              {item.label}
            </ComboboxItem>
          )}
        </ComboboxList>
      </ComboboxContent>
    </Combobox>
  );
}
