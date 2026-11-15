import { defineStore } from "pinia";
import { ref, watch } from "vue";

export type AppearanceTheme = "deep" | "shallow";

function storedValue(key: string) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export const useAppearanceStore = defineStore("appearance", () => {
  const savedTheme = storedValue("shipwiz.theme");
  const theme = ref<AppearanceTheme>(
    savedTheme === "shallow" ? "shallow" : "deep",
  );
  const collapsed = ref(storedValue("shipwiz.sidebar") === "collapsed");
  const logTail = ref(Number(storedValue("shipwiz.logs.tail")) || 200);
  const logFollow = ref(storedValue("shipwiz.logs.follow") !== "false");

  watch(
    theme,
    (value) => {
      document.documentElement.dataset.theme = value;
      localStorage.setItem("shipwiz.theme", value);
    },
    { immediate: true },
  );
  watch(collapsed, (value) =>
    localStorage.setItem("shipwiz.sidebar", value ? "collapsed" : "expanded"),
  );
  watch(logTail, (value) =>
    localStorage.setItem("shipwiz.logs.tail", String(value)),
  );
  watch(logFollow, (value) =>
    localStorage.setItem("shipwiz.logs.follow", String(value)),
  );

  function setTheme(value: AppearanceTheme) {
    theme.value = value;
  }

  return { theme, collapsed, logTail, logFollow, setTheme };
});
