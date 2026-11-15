import { computed, h, ref } from "vue";
import {
  createDiscreteApi,
  darkTheme,
  type ConfigProviderProps,
  type MessageOptions,
} from "naive-ui";
import {
  Anchor,
  Package,
  Ship,
  ShipWheel,
  Siren,
  TriangleAlert,
} from "lucide-vue-next";

const messageTheme = ref<ConfigProviderProps["theme"]>(null);
const configProviderProps = computed<ConfigProviderProps>(() => ({
  theme: messageTheme.value,
}));
const discrete = createDiscreteApi<"message">(["message"], {
  configProviderProps,
  messageProviderProps: {
    containerClass: "nautical-message-container",
    keepAliveOnHover: true,
    max: 3,
  },
});

export const message = discrete.message;

export function syncMessageTheme(theme: "deep" | "shallow") {
  messageTheme.value = theme === "deep" ? darkTheme : null;
}

function renderLaunchIcon() {
  return h(
    "span",
    { class: "nautical-badge nautical-launch", "aria-hidden": "true" },
    [
      h("span", { class: "nautical-launch__wake nautical-launch__wake--one" }),
      h("span", { class: "nautical-launch__wake nautical-launch__wake--two" }),
      h(Ship, { class: "nautical-launch__ship", size: 27, strokeWidth: 1.8 }),
      h("span", { class: "nautical-launch__sea" }),
    ],
  );
}

function renderAnchorIcon() {
  return h(
    "span",
    { class: "nautical-badge nautical-anchor", "aria-hidden": "true" },
    [
      h("span", {
        class: "nautical-anchor__ripple nautical-anchor__ripple--one",
      }),
      h("span", {
        class: "nautical-anchor__ripple nautical-anchor__ripple--two",
      }),
      h(Anchor, { class: "nautical-anchor__icon", size: 25, strokeWidth: 1.9 }),
    ],
  );
}

function renderConnectIcon() {
  return h(
    "span",
    { class: "nautical-badge nautical-connect", "aria-hidden": "true" },
    [
      h("span", {
        class: "nautical-connect__pulse nautical-connect__pulse--one",
      }),
      h("span", {
        class: "nautical-connect__pulse nautical-connect__pulse--two",
      }),
      h(Ship, { class: "nautical-connect__ship", size: 25, strokeWidth: 1.9 }),
    ],
  );
}

function renderHoistIcon() {
  return h(
    "span",
    { class: "nautical-badge nautical-hoist", "aria-hidden": "true" },
    [
      h("span", { class: "nautical-hoist__rope" }),
      h(Package, {
        class: "nautical-hoist__cargo",
        size: 22,
        strokeWidth: 1.8,
      }),
      h("span", { class: "nautical-hoist__deck" }),
    ],
  );
}

function renderDepartIcon() {
  return h(
    "span",
    { class: "nautical-badge nautical-depart", "aria-hidden": "true" },
    [
      h("span", { class: "nautical-depart__wake nautical-depart__wake--one" }),
      h("span", { class: "nautical-depart__wake nautical-depart__wake--two" }),
      h(Ship, { class: "nautical-depart__ship", size: 26, strokeWidth: 1.8 }),
      h("span", { class: "nautical-depart__sea" }),
    ],
  );
}

function renderSteerIcon() {
  return h(
    "span",
    { class: "nautical-badge nautical-steer", "aria-hidden": "true" },
    [
      h("span", { class: "nautical-steer__ring" }),
      h(ShipWheel, {
        class: "nautical-steer__wheel",
        size: 24,
        strokeWidth: 1.9,
      }),
    ],
  );
}

function renderRestartIcon() {
  return h(
    "span",
    { class: "nautical-badge nautical-restart", "aria-hidden": "true" },
    [
      h("span", { class: "nautical-restart__ring" }),
      h(Ship, { class: "nautical-restart__ship", size: 26, strokeWidth: 1.8 }),
      h("span", { class: "nautical-restart__sea" }),
    ],
  );
}

function renderAlertIcon() {
  return h(
    "span",
    { class: "nautical-badge nautical-alert", "aria-hidden": "true" },
    [
      h("span", { class: "nautical-alert__halo nautical-alert__halo--one" }),
      h("span", { class: "nautical-alert__halo nautical-alert__halo--two" }),
      h(Siren, { class: "nautical-alert__icon", size: 24, strokeWidth: 1.9 }),
    ],
  );
}

function renderWarnIcon() {
  return h(
    "span",
    { class: "nautical-badge nautical-warn", "aria-hidden": "true" },
    [
      h("span", { class: "nautical-warn__ring" }),
      h(TriangleAlert, {
        class: "nautical-warn__icon",
        size: 23,
        strokeWidth: 1.9,
      }),
      h("span", { class: "nautical-warn__sea" }),
    ],
  );
}

export function launchSuccess(
  content: string,
  options: Omit<MessageOptions, "type" | "render" | "icon"> = {},
) {
  return message.success(content, {
    duration: 4200,
    ...options,
    icon: renderLaunchIcon,
  });
}

export function anchorSuccess(
  content: string,
  options: Omit<MessageOptions, "type" | "render" | "icon"> = {},
) {
  return message.success(content, {
    duration: 3800,
    ...options,
    icon: renderAnchorIcon,
  });
}

export function connectSuccess(
  content: string,
  options: Omit<MessageOptions, "type" | "render" | "icon"> = {},
) {
  return message.success(content, {
    duration: 3600,
    ...options,
    icon: renderConnectIcon,
  });
}

export function hoistSuccess(
  content: string,
  options: Omit<MessageOptions, "type" | "render" | "icon"> = {},
) {
  return message.success(content, {
    duration: 3600,
    ...options,
    icon: renderHoistIcon,
  });
}

export function departSuccess(
  content: string,
  options: Omit<MessageOptions, "type" | "render" | "icon"> = {},
) {
  return message.success(content, {
    duration: 3800,
    ...options,
    icon: renderDepartIcon,
  });
}

export function steerSuccess(
  content: string,
  options: Omit<MessageOptions, "type" | "render" | "icon"> = {},
) {
  return message.success(content, {
    duration: 3600,
    ...options,
    icon: renderSteerIcon,
  });
}

export function restartSuccess(
  content: string,
  options: Omit<MessageOptions, "type" | "render" | "icon"> = {},
) {
  return message.success(content, {
    duration: 4000,
    ...options,
    icon: renderRestartIcon,
  });
}

export function alertError(
  content: string,
  options: Omit<MessageOptions, "type" | "render" | "icon"> = {},
) {
  return message.error(content, {
    duration: 5600,
    ...options,
    icon: renderAlertIcon,
  });
}

export function alertWarning(
  content: string,
  options: Omit<MessageOptions, "type" | "render" | "icon"> = {},
) {
  return message.warning(content, {
    duration: 4400,
    ...options,
    icon: renderWarnIcon,
  });
}
