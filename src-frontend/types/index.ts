export type NodeType = "local" | "remote";
export type NodeConnection = "local" | "tls" | "ssh";
export type SshAuthMethod = "password" | "privateKey";
export type NodeStatus = "online" | "offline";

export interface ShipNode {
  id: string;
  name: string;
  alias: string;
  type: NodeType;
  connection?: NodeConnection;
  host: string;
  port: number;
  tls: boolean;
  certPath?: string | null;
  username?: string | null;
  authMethod?: SshAuthMethod | null;
  privateKeyPath?: string | null;
  remoteSocket?: string | null;
  credentialId?: string | null;
  lastSuccessTime?: number | null;
  status: NodeStatus;
  createTime: number;
}

export interface NodeInput {
  name: string;
  alias: string;
  type: NodeType;
  connection?: NodeConnection;
  host: string;
  port: number;
  tls: boolean;
  certPath?: string | null;
  username?: string | null;
  authMethod?: SshAuthMethod | null;
  password?: string | null;
  privateKey?: string | null;
  privateKeyPath?: string | null;
  passphrase?: string | null;
  remoteSocket?: string | null;
  clearCredential?: boolean;
}

export interface FleetState {
  nodes: ShipNode[];
  currentNodeId: string | null;
}

export interface ContainerPort {
  privatePort: number;
  publicPort: number | null;
  protocol: string;
  hostIp: string | null;
}

export interface ContainerInfo {
  id: string;
  shortId: string;
  name: string;
  image: string;
  imageId: string;
  state: string;
  status: string;
  created: number;
  ports: ContainerPort[];
  cpuPercent: number;
  memoryUsage: number;
  memoryLimit: number;
  statsAvailable: boolean;
}

export interface PortInput {
  hostPort: number;
  containerPort: number;
  protocol: string;
  hostIp: string;
}

export interface VolumeInput {
  source: string;
  target: string;
  readOnly: boolean;
}

export interface CreateContainerInput {
  name: string;
  image: string;
  command: string;
  env: string[];
  ports: PortInput[];
  volumes: VolumeInput[];
  restartPolicy: string;
  network: string;
  autoRemove: boolean;
  startAfterCreate: boolean;
}

export interface ExecResult {
  output: string;
  exitCode: number | null;
}

export interface FleetOverview {
  containers: number;
  running: number;
  images: number;
  unavailable: number;
}

export interface ImageInfo {
  id: string;
  shortId: string;
  repoTags: string[];
  repoDigests: string[];
  size: number;
  created: number;
  containers: number;
}

export interface ImageSearchResult {
  name: string;
  description: string;
  starCount: number;
  isOfficial: boolean;
}

export interface ImageOperationResult {
  reference: string;
  output: string;
}

export interface RegistrySource {
  id: string;
  name: string;
  url: string;
  isDefault: boolean;
}

export interface RegistryInput {
  name: string;
  url: string;
  isDefault: boolean;
}

export interface ComposeProject {
  id: string;
  name: string;
  yaml: string;
  createdTime: number;
  updatedTime: number;
}

export interface ComposeTemplate {
  id: string;
  title: string;
  desc: string;
  tag: string[];
  yaml: string;
  builtIn: boolean;
}

export interface ComposeDeployment {
  name: string;
  containers: number;
  running: number;
  services: string[];
  state: "running" | "partial" | "stopped" | string;
}

export interface ComposeValidation {
  services: string[];
}

export interface ComposeCommandResult {
  project: string;
  output: string;
}

export interface ApiResponse<T> {
  code: number;
  msg: string;
  data: T | null;
}
