import { useId } from "react";

import { Checkbox } from "@voya/ui/components/checkbox";
import { Input } from "@voya/ui/components/input";
import { Label } from "@voya/ui/components/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@voya/ui/components/select";
import { Textarea } from "@voya/ui/components/textarea";
import { useI18n } from "@voya/i18n/use-i18n";
import { cn } from "@voya/ui/lib/utils";

import { DNS_STRATEGIES, EMPTY_SELECT_VALUE } from "./dns-constants";

export function CheckboxField({
  checked,
  className,
  disabled = false,
  label,
  onChange,
}: {
  checked: boolean;
  className?: string;
  disabled?: boolean;
  label: string;
  onChange: (value: boolean) => void;
}) {
  const id = useId();

  return (
    <div className={cn("flex items-center gap-2", disabled ? "opacity-55" : "", className)}>
      <Checkbox
        checked={checked}
        disabled={disabled}
        id={id}
        onCheckedChange={(nextChecked) => onChange(nextChecked === true)}
      />
      <Label className={cn("text-sm", disabled ? "cursor-not-allowed" : "cursor-pointer")} htmlFor={id}>
        {label}
      </Label>
    </div>
  );
}

export function TextField({
  error,
  label,
  onChange,
  value,
}: {
  error?: string;
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const id = useId();
  const errorId = `${id}-error`;

  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Input
        aria-describedby={error ? errorId : undefined}
        aria-invalid={error ? true : undefined}
        id={id}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
      {error ? (
        <span className="text-xs text-destructive" id={errorId}>
          {error}
        </span>
      ) : null}
    </div>
  );
}

export function SelectField({
  label,
  onChange,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const { t } = useI18n();
  const id = useId();
  const selectValue = value === "" ? EMPTY_SELECT_VALUE : value;

  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Select
        onValueChange={(nextValue) => onChange(nextValue === EMPTY_SELECT_VALUE ? "" : nextValue)}
        value={selectValue}
      >
        <SelectTrigger className="w-full" id={id}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {DNS_STRATEGIES.map((strategy) => (
            <SelectItem key={strategy || EMPTY_SELECT_VALUE} value={strategy || EMPTY_SELECT_VALUE}>
              {strategy || t("panes.routing.defaultValue")}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

export function TextAreaField({
  error,
  label,
  onChange,
  value,
}: {
  error?: string;
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const id = useId();
  const errorId = `${id}-error`;

  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Textarea
        aria-describedby={error ? errorId : undefined}
        aria-invalid={error ? true : undefined}
        className="min-h-24 resize-y"
        id={id}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
      {error ? (
        <span className="text-xs text-destructive" id={errorId}>
          {error}
        </span>
      ) : null}
    </div>
  );
}
