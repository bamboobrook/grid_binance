'use client';

import React, { ReactNode } from 'react';
import { Sidebar } from './sidebar';
import { Topbar } from './topbar';

interface ModernShellProps {
  children: ReactNode;
}

export function ModernShell({ children }: ModernShellProps) {
  return (
    <div className="flex h-screen overflow-hidden bg-background text-foreground font-sans">
      <Sidebar />
      
      <div className="flex-1 flex flex-col min-w-0">
        <Topbar />
        
        <main className="flex-1 overflow-y-auto p-4 md:p-6 bg-background">
          <div className="max-w-[1600px] mx-auto w-full h-full">
            {children}
          </div>
        </main>
      </div>
    </div>
  );
}
