import { z } from "zod";

// Issue messages are translation keys (see forms/zod-errors.ts); the dialog
// renders them through `t`, so nothing here may be a display string.

/** The URL forms the backend accepts: `http(s)://` plus anything but spaces. */
const SUBSCRIPTION_URL = /^https?:\/\/\S+$/i;

/** `autoUpdateIntervalMinutes` is an `i32` in the database; hours must fit. */
const MAX_INTERVAL_MINUTES = 2_147_483_647;

/** The subscription dialog's client-side checks: a named source, a fetchable URL, and a sane interval. */
export const subscriptionFormSchema = z
  .object({
    enabled: z.boolean(),
    remarks: z.string().trim().min(1, "subscriptions.validation.name"),
    url: z.string().trim().regex(SUBSCRIPTION_URL, "subscriptions.validation.url"),
    hours: z.string(),
  })
  .superRefine((value, context) => {
    if (!value.enabled) {
      return;
    }
    const hours = Number(value.hours);
    if (
      !value.hours.trim() ||
      !Number.isFinite(hours) ||
      hours <= 0 ||
      hours * 60 > MAX_INTERVAL_MINUTES
    ) {
      context.addIssue({
        code: "custom",
        path: ["hours"],
        message: "subscriptions.validation.interval",
      });
    }
  });
