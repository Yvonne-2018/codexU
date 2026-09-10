import type { ProjectBoard as ProjectBoardData } from '../types/models';
import { ProjectActivityOverview, ProjectBoard } from './ProjectBoard';

interface ProjectsPanelProps {
  projectBoard: ProjectBoardData | null;
}

export function ProjectsPanel({ projectBoard }: ProjectsPanelProps) {
  return (
    <section className="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <ProjectBoard projectBoard={projectBoard} />
      <ProjectActivityOverview projectBoard={projectBoard} />
    </section>
  );
}
