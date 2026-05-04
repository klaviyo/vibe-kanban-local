import { PROJECTS_SHAPE, type Project } from 'shared/remote-types';
import { type OrganizationWithRole } from 'shared/types';
import { organizationsApi } from '@/shared/lib/api';
import { fetchShapeRows } from '@/shared/lib/electric/fetchShape';
import { getFirstProjectByOrder } from '@/shared/lib/projectOrder';
import type { AppDestination } from '@/shared/lib/routes/appNavigation';

const FIRST_PROJECT_LOOKUP_TIMEOUT_MS = 3000;

function getFirstOrganization(
  organizations: OrganizationWithRole[]
): OrganizationWithRole | null {
  if (organizations.length === 0) {
    return null;
  }

  return organizations[0];
}

async function getProjectsInOrganization(
  organizationId: string
): Promise<Project[] | null> {
  const fetchPromise = fetchShapeRows(PROJECTS_SHAPE, {
    organization_id: organizationId,
  }).catch(() => null);

  const timeoutPromise = new Promise<Project[] | null>((resolve) => {
    window.setTimeout(() => resolve(null), FIRST_PROJECT_LOOKUP_TIMEOUT_MS);
  });

  return Promise.race([fetchPromise, timeoutPromise]);
}

export async function getFirstProjectDestination(
  setSelectedOrgId: (orgId: string | null) => void,
  savedOrgId?: string | null,
  savedProjectId?: string | null
): Promise<AppDestination | null> {
  try {
    const organizationsResponse = await organizationsApi.getUserOrganizations();
    const organizations = organizationsResponse.organizations ?? [];

    // Prefer saved org if it still exists, otherwise fall back to first org
    const savedOrg = savedOrgId
      ? organizations.find((org) => org.id === savedOrgId)
      : null;
    const resolvedOrg = savedOrg ?? getFirstOrganization(organizations);

    if (!resolvedOrg) {
      return null;
    }

    setSelectedOrgId(resolvedOrg.id);

    const projects = await getProjectsInOrganization(resolvedOrg.id);

    // If we have a saved project in the same saved org, use it if still valid
    if (savedProjectId && savedOrg && projects) {
      if (projects.some((p) => p.id === savedProjectId)) {
        return { kind: 'project', projectId: savedProjectId };
      }
    }

    // Fall back to first project by sort order
    const firstProject = projects ? getFirstProjectByOrder(projects) : null;
    if (!firstProject) {
      return null;
    }

    return { kind: 'project', projectId: firstProject.id };
  } catch (error) {
    console.error('Failed to resolve first project destination:', error);
    return null;
  }
}
