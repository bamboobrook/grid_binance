'use client';

import React, { ReactNode } from 'react';
import { Sidebar } from './sidebar';
import { Topbar } from './topbar';
import { useTranslations } from 'next-intl';

interface ModernShellProps {
  children: ReactNode;
}

export function ModernShell({ children }: ModernShellProps) {
  return (
    <div className="flex min-h-screen bg-background text-foreground">
      {/* 3commas style Sidebar */}
      <Sidebar />
      
      <div className="flex-1 flex flex-col min-w-0">
        {/* 3commas style Topbar */}
        <Topbar />
        
        {/* Main Content Area */}
        <main className="flex-1 overflow-y-auto p-6 space-y-6 bg-muted/30">
          <div className="max-w-7xl mx-auto w-full">
            {children}
          </div>
        </main>
      </div>
    </div>
  );
}
