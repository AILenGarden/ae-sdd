'use client';

import { usePathname } from 'next/navigation';

export const STATIC_SHELL_SEGMENT = '__shell__';

function decodeSegment(value: string | undefined): string | null {
  if (!value || value === STATIC_SHELL_SEGMENT) return null;
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function pathSegments(pathname: string | null): string[] {
  return (pathname ?? '')
    .split('/')
    .filter(Boolean)
    .filter((segment) => segment !== 'studio');
}

export function projectRefFromPathname(pathname: string | null): string | null {
  const segments = pathSegments(pathname);
  const projectIndex = segments.indexOf('project');
  return projectIndex >= 0 ? decodeSegment(segments[projectIndex + 1]) : null;
}

export function segmentAfterFromPathname(pathname: string | null, marker: string): string | null {
  const segments = pathSegments(pathname);
  const markerIndex = segments.indexOf(marker);
  return markerIndex >= 0 ? decodeSegment(segments[markerIndex + 1]) : null;
}

export function useProjectRef(paramRef?: string | null): string {
  const pathname = usePathname();
  if (paramRef && paramRef !== STATIC_SHELL_SEGMENT) return paramRef;
  return projectRefFromPathname(pathname) ?? paramRef ?? '';
}

export function useRouteSegmentAfter(marker: string, paramValue?: string | null): string {
  const pathname = usePathname();
  if (paramValue && paramValue !== STATIC_SHELL_SEGMENT) {
    try {
      return decodeURIComponent(paramValue);
    } catch {
      return paramValue;
    }
  }
  return segmentAfterFromPathname(pathname, marker) ?? paramValue ?? '';
}
