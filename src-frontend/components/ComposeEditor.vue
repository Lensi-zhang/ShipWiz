<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import * as monaco from "monaco-editor";
import { configureMonacoYaml } from "monaco-yaml";
import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";
import YamlWorker from "../yaml.worker?worker";

const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{
  "update:modelValue": [value: string];
  validation: [errors: number];
}>();

const host = ref<HTMLDivElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let model: monaco.editor.ITextModel | null = null;
let markerListener: monaco.IDisposable | null = null;
let yamlSupport: ReturnType<typeof configureMonacoYaml> | null = null;

globalThis.MonacoEnvironment = {
  getWorker(_moduleId, label) {
    return label === "yaml" ? new YamlWorker() : new EditorWorker();
  },
};

monaco.editor.defineTheme("shipwiz-compose", {
  base: "vs-dark",
  inherit: true,
  rules: [
    { token: "key", foreground: "8FCAB8" },
    { token: "string", foreground: "D6D7B2" },
    { token: "number", foreground: "D6A77A" },
    { token: "keyword", foreground: "83B6D0" },
    { token: "comment", foreground: "607D80", fontStyle: "italic" },
  ],
  colors: {
    "editor.background": "#132F37",
    "editor.foreground": "#D7E5E1",
    "editor.lineHighlightBackground": "#183842",
    "editorLineNumber.foreground": "#54747A",
    "editorLineNumber.activeForeground": "#8FB9AD",
    "editorCursor.foreground": "#B8DDCE",
    "editor.selectionBackground": "#28525A",
    "editorIndentGuide.background1": "#24434A",
    "editorIndentGuide.activeBackground1": "#3C6267",
  },
});

function updateMarkerCount() {
  if (!model) return;
  const errors = monaco.editor
    .getModelMarkers({ resource: model.uri })
    .filter((marker) => marker.severity === monaco.MarkerSeverity.Error).length;
  emit("validation", errors);
}

onMounted(() => {
  if (!host.value) return;
  yamlSupport = configureMonacoYaml(monaco, {
    enableSchemaRequest: false,
    completion: true,
    format: {},
    hover: true,
    validate: true,
    schemas: [
      {
        uri: "inmemory://shipwiz/compose.schema.json",
        fileMatch: ["**/compose.yaml"],
        schema: {
          $schema: "http://json-schema.org/draft-07/schema#",
          title: "ShipWiz 编队指令",
          type: "object",
          required: ["services"],
          properties: {
            name: { type: "string" },
            services: {
              type: "object",
              minProperties: 1,
              additionalProperties: {
                type: "object",
                properties: {
                  image: { type: "string" },
                  build: { type: ["string", "object"] },
                  command: { type: ["string", "array"] },
                  environment: { type: ["object", "array"] },
                  ports: { type: "array" },
                  volumes: { type: "array" },
                  networks: { type: ["object", "array"] },
                  restart: { type: "string" },
                },
              },
            },
            volumes: { type: "object" },
            networks: { type: "object" },
            secrets: { type: "object" },
            configs: { type: "object" },
          },
        },
      },
    ],
  });
  model = monaco.editor.createModel(
    props.modelValue,
    "yaml",
    monaco.Uri.parse("file:///shipwiz/compose.yaml"),
  );
  editor = monaco.editor.create(host.value, {
    model,
    theme: "shipwiz-compose",
    automaticLayout: true,
    fontSize: 13,
    fontFamily: '"Cascadia Code", "SFMono-Regular", Consolas, monospace',
    lineHeight: 22,
    minimap: { enabled: false },
    padding: { top: 18, bottom: 18 },
    scrollBeyondLastLine: false,
    lineNumbersMinChars: 3,
    fixedOverflowWidgets: true,
    renderLineHighlight: "all",
    scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
  });
  editor.onDidChangeModelContent(() =>
    emit("update:modelValue", model?.getValue() ?? ""),
  );
  markerListener = monaco.editor.onDidChangeMarkers((resources) => {
    if (
      model &&
      resources.some(
        (resource) => resource.toString() === model?.uri.toString(),
      )
    ) {
      updateMarkerCount();
    }
  });
});

watch(
  () => props.modelValue,
  (value) => {
    if (model && value !== model.getValue()) model.setValue(value);
  },
);

onBeforeUnmount(() => {
  markerListener?.dispose();
  editor?.dispose();
  model?.dispose();
  (yamlSupport as unknown as monaco.IDisposable | null)?.dispose();
});
</script>

<template>
  <div ref="host" class="compose-editor" />
</template>

<style scoped>
.compose-editor {
  width: 100%;
  height: 100%;
  min-height: 420px;
  color: var(--sw-terminal-text);
  background: var(--sw-terminal-bg);
}
</style>
