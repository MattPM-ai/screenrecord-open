/**
 * Privacy Notice Component
 * 
 * Explains what data is captured, why, and who can see it.
 * Displays on first launch and can be dismissed.
 */

"use client";

import { useState } from "react";
import { Shield, X, Eye, Lock, Database } from "lucide-react";

interface PrivacyNoticeProps {
  onDismiss: () => void;
  onDontShowAgain: () => void;
}

export function PrivacyNotice({ onDismiss, onDontShowAgain }: PrivacyNoticeProps) {
  const [dontShowAgain, setDontShowAgain] = useState(false);

  const handleDismiss = () => {
    if (dontShowAgain) {
      onDontShowAgain();
    } else {
      onDismiss();
    }
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 z-50 flex items-center justify-center p-4">
      <div className="bg-white rounded-lg shadow-xl max-w-3xl w-full max-h-[90vh] overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-gray-200 bg-blue-50">
          <div className="flex items-center gap-3">
            <Shield className="w-8 h-8 text-blue-600" />
            <h2 className="text-2xl font-bold text-gray-900">Privacy & Data Capture</h2>
          </div>
          <button
            onClick={handleDismiss}
            className="p-2 hover:bg-blue-100 rounded-lg transition-colors"
            aria-label="Close"
          >
            <X className="w-5 h-5 text-gray-600" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 overflow-y-auto max-h-[calc(90vh-200px)]">
          <div className="space-y-6">
            {/* What We Track */}
            <section>
              <div className="flex items-center gap-2 mb-3">
                <Database className="w-5 h-5 text-blue-600" />
                <h3 className="text-lg font-semibold text-gray-900">What We Track</h3>
              </div>
              <div className="bg-gray-50 rounded-lg p-4 space-y-2">
                <div className="flex items-start gap-2">
                  <div className="w-2 h-2 bg-blue-600 rounded-full mt-1.5 shrink-0"></div>
                  <p className="text-gray-700"><span className="font-semibold">Application Names:</span> Which programs you use (e.g., Chrome, VS Code, Terminal)</p>
                </div>
                <div className="flex items-start gap-2">
                  <div className="w-2 h-2 bg-blue-600 rounded-full mt-1.5 shrink-0"></div>
                  <p className="text-gray-700"><span className="font-semibold">Window Titles:</span> The titles of active windows (e.g., document names, webpage titles)</p>
                </div>
                <div className="flex items-start gap-2">
                  <div className="w-2 h-2 bg-blue-600 rounded-full mt-1.5 shrink-0"></div>
                  <p className="text-gray-700"><span className="font-semibold">Activity Status:</span> Whether you're actively using your computer or idle/away</p>
                </div>
                <div className="flex items-start gap-2">
                  <div className="w-2 h-2 bg-blue-600 rounded-full mt-1.5 shrink-0"></div>
                  <p className="text-gray-700"><span className="font-semibold">Timestamps:</span> When each activity occurs and how long it lasts</p>
                </div>
              </div>
            </section>

            {/* What We DON'T Track */}
            <section>
              <h3 className="text-lg font-semibold text-gray-900 mb-3">What We DON'T Track</h3>
              <div className="bg-green-50 rounded-lg p-4 space-y-2">
                <div className="flex items-start gap-2">
                  <div className="w-2 h-2 bg-green-600 rounded-full mt-1.5 shrink-0"></div>
                  <p className="text-gray-700"><span className="font-semibold">Keystrokes:</span> We do NOT log what you type</p>
                </div>
                <div className="flex items-start gap-2">
                  <div className="w-2 h-2 bg-green-600 rounded-full mt-1.5 shrink-0"></div>
                  <p className="text-gray-700"><span className="font-semibold">Screenshots:</span> Disabled by default. Must be explicitly enabled in settings. Blocks banking apps and password managers automatically.</p>
                </div>
                <div className="flex items-start gap-2">
                  <div className="w-2 h-2 bg-green-600 rounded-full mt-1.5 shrink-0"></div>
                  <p className="text-gray-700"><span className="font-semibold">Content:</span> We do NOT record document content or private messages</p>
                </div>
              </div>
            </section>

            {/* Why We Track */}
            <section>
              <h3 className="text-lg font-semibold text-gray-900 mb-3">Why We Track</h3>
              <div className="bg-yellow-50 rounded-lg p-4">
                <p className="text-gray-700 mb-2">
                  This application helps you understand how you spend your time on your computer by providing:
                </p>
                <ul className="space-y-1 ml-4">
                  <li className="text-gray-700">• Productivity insights and time management data</li>
                  <li className="text-gray-700">• Daily activity summaries and patterns</li>
                  <li className="text-gray-700">• Application usage analytics</li>
                  <li className="text-gray-700">• Personal workflow optimization opportunities</li>
                </ul>
              </div>
            </section>

            {/* Who Can See It */}
            <section>
              <div className="flex items-center gap-2 mb-3">
                <Eye className="w-5 h-5 text-blue-600" />
                <h3 className="text-lg font-semibold text-gray-900">Who Can See Your Data</h3>
              </div>
              <div className="bg-purple-50 rounded-lg p-4">
                <div className="flex items-start gap-3">
                  <Lock className="w-6 h-6 text-purple-600 shrink-0 mt-1" />
                  <div>
                    <p className="text-gray-900 font-semibold mb-2">Only You</p>
                    <p className="text-gray-700">
                      All data is stored <span className="font-semibold">locally on your computer</span>. 
                      Nothing is uploaded to any servers or shared with third parties. You have complete 
                      control and ownership of your activity data.
                    </p>
                  </div>
                </div>
              </div>
            </section>

            {/* Controls */}
            <section>
              <h3 className="text-lg font-semibold text-gray-900 mb-3">Your Controls</h3>
              <div className="bg-gray-50 rounded-lg p-4 text-gray-700">
                <p>You can:</p>
                <ul className="space-y-1 ml-4 mt-2">
                  <li>• Stop tracking at any time by stopping the ActivityWatch server</li>
                  <li>• Delete your data at any time from the application data folder</li>
                  <li>• Configure idle detection thresholds</li>
                  <li>• Categorize apps as productive or unproductive</li>
                </ul>
              </div>
            </section>
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between p-4 border-t border-gray-200 bg-gray-50">
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="dontShowAgain"
              checked={dontShowAgain}
              onChange={(e) => setDontShowAgain(e.target.checked)}
              className="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500"
            />
            <label htmlFor="dontShowAgain" className="text-sm text-gray-700">
              Don't show this again
            </label>
          </div>
          <button
            onClick={handleDismiss}
            className="px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors font-medium"
          >
            I Understand
          </button>
        </div>
      </div>
    </div>
  );
}

