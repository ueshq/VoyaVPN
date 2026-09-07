/**
 * Query keys shared by more than one feature.
 *
 * Both the Settings surface and the DNS pane write the backend's DNS block —
 * the surface as part of the whole settings bundle, the pane through its own
 * command pair — so each has to know the other's cache key. Keeping both keys in
 * one leaf module lets them do that without a feature-to-feature import cycle.
 */
export const APP_SETTINGS_QUERY_KEY = ["app-settings"] as const;

export const DNS_QUERY_KEY = ["dns"] as const;
