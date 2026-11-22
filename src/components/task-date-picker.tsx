import { useId, useRef, useState } from "react";
import { Bell, CalendarDays } from "lucide-react";
import { zhCN } from "react-day-picker/locale";
import { Calendar } from "@/components/ui/calendar";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { OptionCombobox } from "@/components/option-combobox";
import { dateKey } from "@/model";

const twoDigits = (n: number) => String(n).padStart(2, "0");
const clockOptions = (count: number) =>
  Array.from({ length: count }, (_, n) => ({
    value: twoDigits(n),
    label: twoDigits(n),
  }));
const hours = clockOptions(24);
const minutes = clockOptions(60);

export function TaskDatePicker({
  kind,
  value,
  onChange,
}: {
  kind: "due" | "reminder";
  value: string;
  onChange: (value: string) => void;
}) {
  const timed = kind === "reminder";
  const label = timed ? "提醒" : "截止日期";
  const selected = value
    ? new Date(timed ? value : value + "T12:00:00")
    : undefined;
  const [open, setOpen] = useState(false);
  const [day, setDay] = useState<Date>();
  const [hour, setHour] = useState("");
  const [minute, setMinute] = useState("");
  const content = useRef<HTMLDivElement>(null);
  const id = useId();

  function show(next: boolean) {
    if (next) {
      const initial =
        selected ?? new Date(Math.ceil(Date.now() / 60000) * 60000);
      setDay(timed ? initial : selected);
      setHour(twoDigits(initial.getHours()));
      setMinute(twoDigits(initial.getMinutes()));
    }
    setOpen(next);
  }

  function commit(next: string) {
    if (value !== next) onChange(next);
    setOpen(false);
  }

  const local = day && new Date(dateKey(day) + "T" + hour + ":" + minute);
  // A daylight-saving transition can skip a local wall-clock time.
  const validTime =
    local &&
    day &&
    dateKey(local) === dateKey(day) &&
    local.getHours() === Number(hour) &&
    local.getMinutes() === Number(minute);

  function confirmReminder() {
    if (!local || !validTime) return;
    if (
      selected &&
      day &&
      dateKey(selected) === dateKey(day) &&
      selected.getHours() === Number(hour) &&
      selected.getMinutes() === Number(minute)
    ) {
      setOpen(false);
    } else commit(local.toISOString());
  }

  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <Popover open={open} onOpenChange={show}>
        <PopoverTrigger asChild>
          <Button
            id={id}
            variant="outline"
            aria-label={label}
            aria-describedby={id + "-value"}
            className="w-full justify-start text-left font-normal"
          >
            {timed ? <Bell /> : <CalendarDays />}
            <span id={id + "-value"}>
              {selected
                ? dateKey(selected) +
                  (timed
                    ? " " +
                      twoDigits(selected.getHours()) +
                      ":" +
                      twoDigits(selected.getMinutes())
                    : "")
                : timed
                  ? "设置提醒"
                  : "选择日期"}
            </span>
          </Button>
        </PopoverTrigger>
        <PopoverContent
          ref={content}
          aria-label={label}
          collisionPadding={8}
          className="w-auto max-h-(--radix-popover-content-available-height) max-w-[calc(100vw-1rem)] overflow-y-auto p-0"
          onEscapeKeyDown={(e) => {
            if (
              content.current?.querySelector(
                '[data-slot="combobox-trigger"][aria-expanded="true"], [data-slot="combobox-content"]',
              )
            )
              e.preventDefault();
          }}
        >
          <Calendar
            mode="single"
            required
            locale={zhCN}
            autoFocus
            selected={day}
            defaultMonth={day}
            labels={{
              labelNext: () => "下个月",
              labelPrevious: () => "上个月",
            }}
            onSelect={(date) => (timed ? setDay(date) : commit(dateKey(date)))}
          />
          {timed && (
            <div className="flex items-center gap-2 border-t px-3 py-2">
              <span className="mr-auto text-sm">时间</span>
              <OptionCombobox
                label="小时"
                compact
                options={hours}
                value={hour}
                onValueChange={setHour}
                container={content}
              />
              <span aria-hidden="true">:</span>
              <OptionCombobox
                label="分钟"
                compact
                options={minutes}
                value={minute}
                onValueChange={setMinute}
                container={content}
              />
            </div>
          )}
          {timed && !validTime && (
            <p role="status" className="px-3 text-sm text-destructive">
              该本地时间不存在，请另选时间。
            </p>
          )}
          <div className="flex items-center justify-between border-t p-2">
            <Button
              variant="ghost"
              size="sm"
              disabled={!value}
              onClick={() => commit("")}
            >
              清除
            </Button>
            {timed && (
              <Button size="sm" disabled={!validTime} onClick={confirmReminder}>
                确定
              </Button>
            )}
          </div>
        </PopoverContent>
      </Popover>
    </div>
  );
}
