import { formatProxyEndpoint } from "../utils/format";

/**
 * IP 地址列：仅展示 `host:port`，不展示 scheme / 账号密码。
 */
export interface ProxyAddressCellProps {
  address: string;
}

export function ProxyAddressCell({ address }: ProxyAddressCellProps) {
  const endpoint = formatProxyEndpoint(address);
  return (
    <span className="truncate font-mono text-xs">{endpoint}</span>
  );
}
