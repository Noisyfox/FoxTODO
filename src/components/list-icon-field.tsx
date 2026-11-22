import { useId } from "react";
import emojiRegex from "emoji-regex";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

const singleEmoji = new RegExp(`^(?:${emojiRegex().source})$`);
const suggestions = [
  ["📋", "清单"],
  ["💼", "工作"],
  ["🛒", "购物"],
  ["📚", "阅读"],
  ["🏠", "家庭"],
  ["✈️", "旅行"],
] as const;

export function ListIconField({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const id = useId();
  const icon = value.trim();
  const error = icon && !singleEmoji.test(icon) ? "请输入一个表情符号" : "";
  return (
    <div className="field">
      <label htmlFor={id}>清单图标</label>
      <div className="flex items-center gap-2">
        <Input
          id={id}
          value={value}
          placeholder="输入或粘贴一个表情"
          autoComplete="off"
          aria-invalid={!!error}
          aria-describedby={error ? `${id}-error` : undefined}
          ref={(input) => { input?.setCustomValidity(error); }}
          onFocus={(event) => event.currentTarget.select()}
          onChange={(event) => onChange(event.target.value)}
        />
        <Button type="button" variant="ghost" disabled={!value} onClick={() => onChange("")}>
          清除图标
        </Button>
      </div>
      {error && <span id={`${id}-error`} className="text-destructive">{error}</span>}
      <div role="group" aria-label="常用图标" className="grid grid-cols-6 gap-1">
        {suggestions.map(([emoji, label]) => (
          <Button
            key={emoji}
            type="button"
            variant={icon === emoji ? "secondary" : "ghost"}
            aria-label={label}
            aria-pressed={icon === emoji}
            title={label}
            className="px-0 text-xl"
            onClick={() => onChange(emoji)}
          >
            {emoji}
          </Button>
        ))}
      </div>
    </div>
  );
}
