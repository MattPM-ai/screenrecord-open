"use client";

import { Info } from "lucide-react";
import {
  SyncStatistics,
  getStatusColor,
} from "@/lib/collectorClient";

type CollectorStatusProps = {
  statistics: SyncStatistics | null;
};

export function CollectorStatus({ statistics }: CollectorStatusProps) {
  if (!statistics) {
    return (
      <div className="inline-flex items-center gap-2 px-3 py-1.5 bg-gray-100 rounded-full">
        <div className="w-2 h-2 rounded-full bg-gray-400" />
        <span className="text-xs font-medium text-gray-600">Not Configured</span>
      </div>
    );
  }

  const statusColor = getStatusColor(statistics.connection_status);
  const hasError = statistics.connection_status.type === "Error";
  const errorMessage = statistics.connection_status.type === "Error" 
    ? statistics.connection_status.message 
    : null;
  const isConnecting =
    statistics.connection_status.type === "Connecting" ||
    statistics.connection_status.type === "Authenticating";

  // Simple status text without the error details
  const statusText = hasError
    ? "Error"
    : statistics.connection_status.type === "Connecting"
    ? "Connecting..."
    : statistics.connection_status.type === "Authenticating"
    ? "Authenticating..."
    : statistics.connection_status.type;

  return (
    <div className="inline-flex items-center gap-1 px-3 py-1.5 bg-gray-100 rounded-full">
      <div
        className={`w-2 h-2 rounded-full ${
          statusColor === "green"
            ? "bg-green-500"
            : statusColor === "yellow"
            ? "bg-yellow-500"
            : statusColor === "red"
            ? "bg-red-500"
            : "bg-gray-400"
        } ${isConnecting ? "animate-pulse" : ""}`}
      />
      <span className="text-xs font-medium text-gray-700">{statusText}</span>
      {hasError && errorMessage && (
        <div className="relative group">
          <Info className="w-3.5 h-3.5 text-red-500 cursor-help" />
          <p className="absolute bottom-full w-64 left-0 mb-2 px-3 py-2 bg-gray-100 text-black text-xs rounded-sm opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all duration-200 z-50 max-w-xs shadow-sm">
            {errorMessage}
          </p>
        </div>
      )}
    </div>
  );
}
