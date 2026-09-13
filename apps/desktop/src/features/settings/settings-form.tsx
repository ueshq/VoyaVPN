import {
  createContext,
  useContext,
  useId,
  type ComponentProps,
  type ReactNode,
} from "react";

import {
  CheckboxField,
  FieldLayout,
  SelectField as SharedSelectField,
  TextField as SharedTextField,
} from "@voya/ui/components/form-fields";
import { cn } from "@voya/ui/lib/utils";
import { useI18n } from "@voya/i18n/use-i18n";
import { pageSurfaceClassName } from "@/components/app-shell/page-section";

const FieldErrors = createContext<Record<string, string>>({});

export function SettingsFields({
  children,
  errors,
}: {
  children: ReactNode;
  errors: Record<string, string>;
}) {
  return <FieldErrors value={errors}>{children}</FieldErrors>;
}

export function SettingsGroup({
  children,
  className,
  title,
  actions,
}: {
  children: ReactNode;
  className?: string;
  title: string;
  actions?: ReactNode;
}) {
  const id = useId();
  return (
    <section
      aria-labelledby={id}
      className={cn(
        pageSurfaceClassName,
        "@container",
        className,
      )}
    >
      <div className="flex min-h-12 flex-wrap items-center gap-2 px-4 py-3">
        <h2 className="text-section font-semibold" id={id}>
          {title}
        </h2>
        {actions ? (
          <div className="ms-auto flex items-center gap-2">{actions}</div>
        ) : null}
      </div>
      <div className="grid gap-4 p-4">{children}</div>
    </section>
  );
}

export function SettingsRow({
  children,
  className,
  description,
  htmlFor,
  label,
  error,
}: {
  children: ReactNode;
  className?: string;
  description?: ReactNode;
  htmlFor?: string;
  label?: ReactNode;
  error?: string;
}) {
  const id = useId();
  return (
    <FieldLayout
      className={className}
      description={description}
      error={error}
      group={!htmlFor}
      id={htmlFor ?? id}
      label={label}
      layout="row"
    >
      {children}
    </FieldLayout>
  );
}

export function SettingsCheckbox({
  field,
  onCheckedChange,
  ...props
}: Omit<ComponentProps<typeof CheckboxField>, "onChange"> & {
  field?: string;
  onCheckedChange: (checked: boolean) => void;
}) {
  const errors = useContext(FieldErrors);
  return (
    <CheckboxField
      {...props}
      error={field ? errors[field] : undefined}
      onChange={onCheckedChange}
    />
  );
}

type TextProps = Omit<
  ComponentProps<typeof SharedTextField>,
  "layout" | "commitOnBlur"
> & { field: string };

export function TextField({ field, ...props }: TextProps) {
  const errors = useContext(FieldErrors);
  return (
    <SharedTextField
      {...props}
      commitOnBlur
      error={errors[field]}
      layout="row"
    />
  );
}

export function NumberField({
  defaultValue,
  description,
  nullable = false,
  onChange,
  value,
  ...props
}: Omit<TextProps, "description" | "onChange" | "value"> & {
  /** Restored when the field is cleared, and named under the field. */
  defaultValue?: number;
  description?: string;
  nullable?: boolean;
  onChange: (value: number | null) => void;
  value: number | null;
}) {
  const { t } = useI18n();
  const hint =
    defaultValue === undefined
      ? description
      : [description, t("settings.defaultValue", { value: defaultValue })]
          .filter(Boolean)
          .join(" ");
  // A typing mistake stays next to the field; it is not a failed save, so it
  // never raises a toast.
  return (
    <TextField
      {...props}
      description={hint}
      inputMode="numeric"
      onChange={(text) =>
        onChange(text.trim() ? Number(text) : (defaultValue ?? null))
      }
      validate={(text) => {
        if (!text.trim())
          return nullable || defaultValue !== undefined
            ? undefined
            : t("validation.textRequired");
        const number = Number(text);
        return Number.isInteger(number) &&
          number >= -2147483648 &&
          number <= 2147483647
          ? undefined
          : t("validation.invalid");
      }}
      value={value === null ? "" : String(value)}
    />
  );
}

export function SelectField({
  field,
  optionLabel = (value) => value,
  options,
  ...props
}: Omit<ComponentProps<typeof SharedSelectField>, "options" | "layout"> & {
  field: string;
  optionLabel?: (value: string) => string;
  options: readonly string[];
}) {
  const errors = useContext(FieldErrors);
  return (
    <SharedSelectField
      {...props}
      error={errors[field]}
      layout="row"
      options={options.map((value) => ({ value, label: optionLabel(value) }))}
    />
  );
}
