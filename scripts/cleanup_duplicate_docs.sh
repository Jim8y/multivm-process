#!/bin/bash

# Cleanup script to consolidate duplicate documentation and reports

echo "🧹 Cleaning up duplicate documentation files..."

# Create archive directory for old reports
mkdir -p docs/archive/old_reports

# Move duplicate status reports to archive (keeping the most recent/comprehensive ones)
echo "📦 Archiving old status reports..."

# Keep FINAL_VERIFICATION_REPORT.md as the authoritative completion status
# Archive older completion reports
mv docs/reports/PROJECT_COMPLETION_STATUS.md docs/archive/old_reports/ 2>/dev/null
mv docs/reports/FINAL_COMPLETION_STATUS.md docs/archive/old_reports/ 2>/dev/null
mv docs/reports/BUILD_COMPLETE_REPORT.md docs/archive/old_reports/ 2>/dev/null
mv docs/reports/BUILD_SUCCESS_FINAL.md docs/archive/old_reports/ 2>/dev/null
mv docs/reports/FINAL_BUILD_STATUS.md docs/archive/old_reports/ 2>/dev/null

# Keep PRODUCTION_READY_STATUS.md, archive the other
mv docs/reports/PRODUCTION_READINESS_REPORT.md docs/archive/old_reports/ 2>/dev/null

# Archive old TODO completion report (replaced by FINAL_VERIFICATION_REPORT)
mv docs/reports/TODO_COMPLETION_REPORT.md docs/archive/old_reports/ 2>/dev/null
mv docs/reports/PLACEHOLDER_ELIMINATION_COMPLETE.md docs/archive/old_reports/ 2>/dev/null

# Clean up duplicate project structure files
echo "🔧 Consolidating project structure documentation..."
# Keep the root PROJECT_STRUCTURE.md as authoritative
mv docs/reports/PROJECT_STRUCTURE.md docs/archive/old_reports/ 2>/dev/null

# Archive redundant cleanup summaries
mv docs/reports/PROJECT_CLEANUP_SUMMARY.md docs/archive/old_reports/ 2>/dev/null
mv docs/reports/PROJECT_ORGANIZATION.md docs/archive/old_reports/ 2>/dev/null

# Clean up crate-specific status files that are redundant
mv crates/multivm-process-manager/PROJECT_STATUS.md docs/archive/old_reports/process_manager_status.md 2>/dev/null

# Remove old symlinks or references that no longer exist
echo "🔗 Cleaning up broken symlinks and references..."
# These files were moved but git still tracks them as untracked
rm -f DOCKER_TESTING.md 2>/dev/null
rm -f docker-compose.multi.yml docker-compose.single.yml 2>/dev/null
rm -f generate-sample-log.sh monitor-blocks.sh monitor-network.sh 2>/dev/null
rm -f run-multivm.sh run-native-single-node.sh run-single-node.sh 2>/dev/null
rm -f simulate-single-node.sh start-and-log.sh start-real-node.sh 2>/dev/null
rm -f test-docker-network.sh 2>/dev/null
rm -f crates/multivm-cli/start-multivm-node.sh 2>/dev/null

echo "📄 Creating consolidated documentation index..."