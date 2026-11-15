<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import {
  NAlert,
  NButton,
  NCheckbox,
  NForm,
  NFormItem,
  NIcon,
  NInput,
  NInputNumber,
  NModal,
  NRadioButton,
  NRadioGroup,
} from "naive-ui";
import { Ship } from "lucide-vue-next";
import { dict } from "../dict";
import { useFleetStore } from "../store/fleet";
import type { NodeConnection, NodeInput, NodeType, ShipNode, SshAuthMethod } from "../types";

const props = defineProps<{ show: boolean; node: ShipNode | null }>();
const emit = defineEmits<{
  "update:show": [value: boolean];
  saved: [mode: "create" | "edit"];
}>();
const fleet = useFleetStore();
const saving = ref(false);
const errors = ref<Record<string, string>>({});
const privateKeySource = ref<"content" | "path">("content");
let initialPrivateKeySource: "content" | "path" = "content";
let resetting = false;

interface FormState {
  name: string;
  alias: string;
  type: NodeType;
  connection: NodeConnection;
  host: string;
  port: number;
  tls: boolean;
  certPath: string;
  username: string;
  authMethod: SshAuthMethod;
  password: string;
  privateKey: string;
  privateKeyPath: string;
  passphrase: string;
  remoteSocket: string;
  clearCredential: boolean;
}

function defaults(): FormState {
  return {
    name: "",
    alias: "",
    type: "local",
    connection: "local",
    host: "localhost",
    port: 0,
    tls: false,
    certPath: "",
    username: "",
    authMethod: "password",
    password: "",
    privateKey: "",
    privateKeyPath: "",
    passphrase: "",
    remoteSocket: "/var/run/docker.sock",
    clearCredential: false,
  };
}

const form = reactive<FormState>(defaults());
const visible = computed({ get: () => props.show, set: (value) => emit("update:show", value) });
const hasStoredCredential = computed(() => Boolean(props.node?.credentialId));
const canReusePassword = computed(() =>
  hasStoredCredential.value && !form.clearCredential && props.node?.authMethod === "password",
);
const canReusePrivateKey = computed(() =>
  hasStoredCredential.value
  && !form.clearCredential
  && props.node?.authMethod === "privateKey"
  && initialPrivateKeySource === "content"
  && privateKeySource.value === "content",
);
const privateKeySourceChanged = computed(() =>
  hasStoredCredential.value
  && props.node?.authMethod === "privateKey"
  && privateKeySource.value !== initialPrivateKeySource,
);

function resetForm(node: ShipNode | null) {
  resetting = true;
  saving.value = false;
  const connection: NodeConnection = node?.connection ?? (node?.type === "remote" ? "tls" : "local");
  initialPrivateKeySource = node?.privateKeyPath ? "path" : "content";
  privateKeySource.value = initialPrivateKeySource;
  Object.assign(form, defaults(), {
    name: node?.name ?? "",
    alias: node?.alias ?? "",
    type: node?.type ?? "local",
    connection,
    host: node?.host ?? "localhost",
    port: node?.port ?? (connection === "ssh" ? 22 : connection === "tls" ? 2376 : 0),
    tls: connection === "tls",
    certPath: node?.certPath ?? "",
    username: node?.username ?? "",
    authMethod: node?.authMethod ?? "password",
    privateKeyPath: node?.privateKeyPath ?? "",
    remoteSocket: node?.remoteSocket ?? "/var/run/docker.sock",
  });
  errors.value = {};
  resetting = false;
}

watch(() => [props.show, props.node] as const, ([show]) => {
  if (show) resetForm(props.node);
}, { immediate: true });

watch(() => form.type, (type, previous) => {
  if (resetting || type === previous) return;
  if (type === "local") {
    form.connection = "local";
    form.host = "localhost";
    form.port = 0;
    form.tls = false;
    form.certPath = "";
  } else {
    form.connection = "tls";
    form.host = "";
    form.port = 2376;
    form.tls = true;
  }
  errors.value = {};
}, { flush: "sync" });

watch(() => form.connection, (connection, previous) => {
  if (resetting || connection === previous) return;
  if (connection === "ssh") {
    form.port = 22;
    form.tls = false;
    form.certPath = "";
    form.remoteSocket = "/var/run/docker.sock";
  } else {
    form.port = connection === "tls" ? 2376 : 0;
    form.tls = connection === "tls";
    form.username = "";
    form.password = "";
    form.privateKey = "";
    form.privateKeyPath = "";
    form.passphrase = "";
    form.clearCredential = false;
    if (connection === "local") form.host = "localhost";
  }
  errors.value = {};
}, { flush: "sync" });

watch(() => form.authMethod, (_method, previous) => {
  if (resetting || _method === previous) return;
  form.password = "";
  form.privateKey = "";
  form.passphrase = "";
  form.clearCredential = false;
  errors.value = {};
}, { flush: "sync" });

watch(privateKeySource, () => {
  if (!resetting) errors.value = {};
});
watch(() => form.clearCredential, (clear) => {
  if (resetting || !clear) return;
  form.password = "";
  form.privateKey = "";
  form.passphrase = "";
});

function hasError(key: string) {
  return Boolean(errors.value[key]);
}

function validate() {
  const next: Record<string, string> = {};
  if (!form.name.trim()) next.name = "请填写舰船名称。";
  if (form.type === "remote") {
    if (!form.host.trim()) next.host = "请填写主机 IP 或域名。";
    else if (form.host.includes("://")) next.host = "仅填写 IP 或域名，不要包含协议前缀。";
    if (!Number.isInteger(form.port) || form.port < 1 || form.port > 65535) {
      next.port = "请输入 1–65535 之间的有效端口。";
    }
  }
  if (form.connection === "tls" && !form.certPath.trim()) {
    next.certPath = "TLS 通道必须填写证书目录。";
  }
  if (form.connection === "ssh") {
    if (!form.username.trim()) next.username = "请填写 SSH 用户名。";
    if (!form.remoteSocket.trim().startsWith("/")) {
      next.remoteSocket = "请输入 Unix Socket 绝对路径，例如 /var/run/docker.sock。";
    }
    if (form.authMethod === "password" && !form.password && !canReusePassword.value) {
      next.password = hasStoredCredential.value ? "请填写新密码，或取消清除凭据以沿用原密码。" : "请填写 SSH 密码。";
    }
    if (form.authMethod === "privateKey") {
      if (privateKeySource.value === "path" && !form.privateKeyPath.trim()) {
        next.privateKeyPath = "请填写本机私钥文件的绝对路径。";
      }
      if (privateKeySource.value === "content" && !form.privateKey.trim() && !canReusePrivateKey.value) {
        next.privateKey = hasStoredCredential.value ? "请粘贴新私钥，或取消清除凭据以沿用原私钥。" : "请粘贴 PEM/OpenSSH 私钥内容。";
      }
    }
  }
  errors.value = next;
  return Object.keys(next).length === 0;
}

async function submit() {
  if (saving.value || !validate()) return;
  saving.value = true;
  const input: NodeInput = {
    name: form.name.trim(),
    alias: form.alias.trim(),
    type: form.type,
    connection: form.connection,
    host: form.host.trim(),
    port: form.type === "local" ? 0 : form.port,
    tls: form.connection === "tls",
    certPath: form.connection === "tls" ? form.certPath.trim() : null,
    clearCredential: form.clearCredential || privateKeySourceChanged.value,
  };
  if (form.connection === "ssh") {
    input.username = form.username.trim();
    input.authMethod = form.authMethod;
    input.remoteSocket = form.remoteSocket.trim();
    if (form.authMethod === "password") {
      input.password = form.password || null;
    } else {
      input.privateKey = privateKeySource.value === "content" ? form.privateKey || null : null;
      input.privateKeyPath = privateKeySource.value === "path" ? form.privateKeyPath.trim() : null;
      input.passphrase = form.passphrase || null;
    }
  }
  try {
    const succeeded = await fleet.save(input, props.node?.id);
    if (!succeeded) return;
    emit("saved", props.node ? "edit" : "create");
    visible.value = false;
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <n-modal
    v-model:show="visible"
    preset="card"
    class="form-modal form-modal--wide node-modal"
    :title="node ? dict.fleet.editTitle : dict.fleet.createTitle"
    :bordered="false"
    :closable="!saving"
    :mask-closable="!saving"
  >
    <template #header-extra><n-icon class="modal-ship" :component="Ship" :size="21" /></template>
    <n-form label-placement="top" :show-require-mark="false" @submit.prevent="submit">
      <div class="form-grid">
        <n-form-item :label="dict.fleet.name" required :validation-status="hasError('name') ? 'error' : undefined" :feedback="errors.name" :show-feedback="hasError('name')">
          <n-input v-model:value="form.name" :placeholder="dict.fleet.namePlaceholder" maxlength="40" />
        </n-form-item>
        <n-form-item :label="dict.fleet.alias">
          <n-input v-model:value="form.alias" :placeholder="dict.fleet.aliasPlaceholder" maxlength="60" />
        </n-form-item>
      </div>

      <n-form-item :label="dict.fleet.type">
        <n-radio-group v-model:value="form.type" class="node-type-options">
          <n-radio-button value="local">{{ dict.fleet.local }}</n-radio-button>
          <n-radio-button value="remote">{{ dict.fleet.remote }}</n-radio-button>
        </n-radio-group>
      </n-form-item>

      <n-alert v-if="form.type === 'local'" type="info" :bordered="false" class="form-alert">
        {{ dict.fleet.localConnection }}
      </n-alert>

      <template v-else>
        <n-form-item label="安全连接通道" required>
          <n-radio-group v-model:value="form.connection" class="node-type-options">
            <n-radio-button value="tls">TLS · Docker API</n-radio-button>
            <n-radio-button value="ssh">SSH · Socket 隧道</n-radio-button>
          </n-radio-group>
        </n-form-item>

        <div class="form-grid remote-grid">
          <n-form-item :label="dict.fleet.host" required :validation-status="hasError('host') ? 'error' : undefined" :feedback="errors.host" :show-feedback="hasError('host')">
            <n-input v-model:value="form.host" placeholder="192.168.1.100 或 ship.example.com" />
          </n-form-item>
          <n-form-item :label="dict.fleet.port" required :validation-status="hasError('port') ? 'error' : undefined" :feedback="errors.port" :show-feedback="hasError('port')">
            <n-input-number v-model:value="form.port" :min="1" :max="65535" :precision="0" :show-button="false" class="port-input" />
          </n-form-item>
        </div>

        <template v-if="form.connection === 'tls'">
          <n-form-item :label="dict.fleet.certPath" required :validation-status="hasError('certPath') ? 'error' : undefined" :feedback="errors.certPath" :show-feedback="hasError('certPath')">
            <n-input v-model:value="form.certPath" :placeholder="dict.fleet.certPlaceholder" />
          </n-form-item>
          <n-alert type="info" :bordered="false" class="form-alert">TLS 使用 ca.pem、cert.pem 和 key.pem，适用于 Linux 与 Windows Docker 引擎。</n-alert>
        </template>

        <template v-else>
          <n-alert type="info" :bordered="false" class="form-alert">
            SSH 隧道转发远端 Unix Socket，仅支持 Linux/Unix 目标；Windows Docker 请使用 TLS。秘密仅保存到系统密钥库。
          </n-alert>
          <div class="form-grid">
            <n-form-item label="SSH 用户名" required :validation-status="hasError('username') ? 'error' : undefined" :feedback="errors.username" :show-feedback="hasError('username')">
              <n-input v-model:value="form.username" placeholder="root 或具有 Docker 权限的用户" />
            </n-form-item>
            <n-form-item label="认证方式" required>
              <n-radio-group v-model:value="form.authMethod" class="node-type-options">
                <n-radio-button value="password">密码</n-radio-button>
                <n-radio-button value="privateKey">私钥</n-radio-button>
              </n-radio-group>
            </n-form-item>
          </div>

          <n-form-item v-if="form.authMethod === 'password'" label="SSH 密码" :required="!canReusePassword" :validation-status="hasError('password') ? 'error' : undefined" :feedback="errors.password" :show-feedback="hasError('password')">
            <n-input v-model:value="form.password" type="password" show-password-on="click" :placeholder="canReusePassword ? '留空沿用系统密钥库中的密码' : '输入 SSH 密码'" />
          </n-form-item>

          <template v-else>
            <n-form-item label="私钥来源" required>
              <n-radio-group v-model:value="privateKeySource" class="node-type-options">
                <n-radio-button value="content">粘贴私钥</n-radio-button>
                <n-radio-button value="path">本机私钥路径</n-radio-button>
              </n-radio-group>
            </n-form-item>
            <n-form-item v-if="privateKeySource === 'content'" label="私钥内容" :required="!canReusePrivateKey" :validation-status="hasError('privateKey') ? 'error' : undefined" :feedback="errors.privateKey" :show-feedback="hasError('privateKey')">
              <n-input v-model:value="form.privateKey" type="textarea" :rows="5" :placeholder="canReusePrivateKey ? '留空沿用系统密钥库中的私钥' : '-----BEGIN OPENSSH PRIVATE KEY-----'" />
            </n-form-item>
            <n-form-item v-else label="本机私钥路径" required :validation-status="hasError('privateKeyPath') ? 'error' : undefined" :feedback="errors.privateKeyPath" :show-feedback="hasError('privateKeyPath')">
              <n-input v-model:value="form.privateKeyPath" placeholder="运行 ShipWiz 的本机私钥文件绝对路径" />
            </n-form-item>
            <n-form-item label="私钥口令（可选）">
              <n-input v-model:value="form.passphrase" type="password" show-password-on="click" placeholder="加密私钥的口令；留空沿用已有口令" />
            </n-form-item>
          </template>

          <n-form-item label="远端 Docker Socket" required :validation-status="hasError('remoteSocket') ? 'error' : undefined" :feedback="errors.remoteSocket" :show-feedback="hasError('remoteSocket')">
            <n-input v-model:value="form.remoteSocket" placeholder="/var/run/docker.sock" />
          </n-form-item>
          <n-checkbox v-if="hasStoredCredential" v-model:checked="form.clearCredential" class="clear-credential">
            清除已保存凭据并重新设置
          </n-checkbox>
          <p v-if="hasStoredCredential" class="credential-hint">
            {{ form.clearCredential || privateKeySourceChanged ? '保存时将清除旧秘密，请提供替换认证信息。' : '秘密不会回显；留空表示沿用系统密钥库中的现有值。' }}
          </p>
        </template>
      </template>
    </n-form>
    <template #footer>
      <div class="modal-footer">
        <n-button :disabled="saving" @click="visible = false">{{ dict.common.cancel }}</n-button>
        <n-button type="primary" :loading="saving" @click="submit">{{ dict.common.save }}</n-button>
      </div>
    </template>
  </n-modal>
</template>

<style scoped>
.form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
.node-type-options { display: flex; width: 100%; }
.node-type-options :deep(.n-radio-button) { flex: 1; min-width: 0; text-align: center; }
.modal-ship { color: var(--sw-primary); }
.remote-grid { grid-template-columns: minmax(0, 1fr) 130px; }
.port-input { width: 100%; }
.form-alert { margin: -4px 0 18px; }
.clear-credential { margin: -4px 0 7px; color: var(--sw-text-secondary); }
.credential-hint { margin: 0 0 18px; color: var(--sw-muted); font-size: 11px; line-height: 1.6; }
.modal-footer { display: flex; justify-content: flex-end; gap: 10px; }
.node-modal :deep(.n-form-item) { flex-shrink: 0; }
@media (max-width: 560px) { .form-grid { grid-template-columns: 1fr; gap: 0; } .remote-grid { grid-template-columns: 1fr; } }
</style>
