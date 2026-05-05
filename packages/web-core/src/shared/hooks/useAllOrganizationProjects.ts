import { useMemo } from 'react';
import { useQueries } from '@tanstack/react-query';

import { fetchShapeRows } from '@/shared/lib/electric/fetchShape';
import { PROJECTS_SHAPE, type Project } from 'shared/remote-types';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { useUserOrganizations } from '@/shared/hooks/useUserOrganizations';

interface UseAllOrganizationProjectsOptions {
  enabled?: boolean;
}

const STALE_TIME_MS = 30 * 1000;

/**
 * Hook that fetches remote projects across ALL user organizations.
 *
 * Uses parallel React Query fetches (one per org) so the results stay in the
 * shared cache and de-duplicate with same-org subscriptions elsewhere.
 */
export function useAllOrganizationProjects(
  options: UseAllOrganizationProjectsOptions = {}
) {
  const { enabled = true } = options;
  const { isSignedIn } = useAuth();
  const { data: orgsData } = useUserOrganizations();

  const orgIds = useMemo(
    () => (orgsData?.organizations ?? []).map((o) => o.id),
    [orgsData?.organizations]
  );

  const queries = useQueries({
    queries: orgIds.map((orgId) => ({
      queryKey: ['shape', PROJECTS_SHAPE.table, { organization_id: orgId }],
      queryFn: () => fetchShapeRows(PROJECTS_SHAPE, { organization_id: orgId }),
      enabled: enabled && isSignedIn && orgIds.length > 0,
      staleTime: STALE_TIME_MS,
    })),
  });

  const data = useMemo<Project[]>(() => {
    const result: Project[] = [];
    for (const q of queries) {
      if (q.data) {
        result.push(...q.data);
      }
    }
    return result;
  }, [queries]);

  const isLoading =
    enabled && isSignedIn && orgIds.length > 0
      ? queries.some((q) => q.isLoading)
      : false;

  return { data, isLoading };
}
