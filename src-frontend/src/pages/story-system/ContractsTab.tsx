import { useState, useEffect } from 'react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import {
  FileText,
  Plus,
  AlertTriangle,
  CheckCircle,
  XCircle,
  ScrollText,
  LayoutDashboard,
} from 'lucide-react';
import { useAppStore } from '@/stores/appStore';
import {
  getContractTree,
  getRuntimeContract,
  createMasterSetting,
  createChapterContract,
  listGenesisRuns,
} from '@/services/tauri';
import type { ContractTree, RuntimeContract, GenesisRun } from '@/services/tauri';
import { parseGenesisStepsJson, countGenesisErrors, sortGenesisErrors } from '@/utils/genesisSteps';
import toast from 'react-hot-toast';

interface ContractsTabProps {
  storyId: string;
  selectedChapter: number;
  onChapterChange: (n: number) => void;
}

export function ContractsTab({ storyId, selectedChapter, onChapterChange }: ContractsTabProps) {
  const currentStory = useAppStore(s => s.currentStory);
  const setCurrentView = useAppStore(s => s.setCurrentView);

  const [contractTree, setContractTree] = useState<ContractTree | null>(null);
  const [runtimeContract, setRuntimeContract] = useState<RuntimeContract | null>(null);
  const [failedRun, setFailedRun] = useState<GenesisRun | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const loadContracts = async () => {
    setIsLoading(true);
    try {
      const [tree, runtime] = await Promise.all([
        getContractTree(storyId),
        getRuntimeContract(storyId, selectedChapter),
      ]);
      setContractTree(tree);
      setRuntimeContract(runtime);
    } catch {
      toast.error('加载合同失败');
    } finally {
      setIsLoading(false);
    }
  };

  const loadFailedRun = async () => {
    try {
      const runs = await listGenesisRuns(20);
      const run = runs.find(r => r.story_id === storyId && r.status === 'failed');
      setFailedRun(run || null);
    } catch {
      // silent fail
    }
  };

  useEffect(() => {
    loadContracts();
    loadFailedRun();
  }, [storyId, selectedChapter]);

  const hasMaster = !!contractTree?.master_setting;
  const hasChapter1 = !!contractTree?.chapters['1'];

  const handleCreateMaster = async () => {
    if (!currentStory) return;
    try {
      await createMasterSetting({
        story_id: currentStory.id,
        genre: currentStory.genre || '小说',
        core_tone: currentStory.tone || '中性',
        pacing_strategy: '正常',
        anti_patterns: [],
        world_rules: [],
      });
      toast.success('世界观合同已创建');
      loadContracts();
    } catch {
      toast.error('创建世界观合同失败');
    }
  };

  const handleCreateChapter = async () => {
    if (!currentStory) return;
    try {
      await createChapterContract({
        story_id: currentStory.id,
        chapter_number: selectedChapter,
        goal: `完成第${selectedChapter}章的情节推进`,
        must_cover_nodes: [],
        forbidden_zones: [],
      });
      toast.success(`第${selectedChapter}章合同已创建`);
      loadContracts();
    } catch {
      toast.error('创建章节合同失败');
    }
  };

  const failedRunErrors = failedRun
    ? sortGenesisErrors(parseGenesisStepsJson(failedRun.steps_json).errors)
    : [];
  const failedRunErrorCounts = failedRun
    ? countGenesisErrors(failedRun.steps_json)
    : { total: 0, warnings: 0, errors: 0 };

  return (
    <div className="space-y-6">
      {/* Contract seeding status cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <Card>
          <CardContent className="p-4 flex items-center justify-between">
            <div className="flex items-center gap-3">
              {hasMaster ? (
                <CheckCircle className="w-5 h-5 text-green-400" />
              ) : (
                <XCircle className="w-5 h-5 text-gray-500" />
              )}
              <div>
                <p className="text-white font-medium">MASTER_SETTING</p>
                <p className="text-gray-400 text-xs">{hasMaster ? '已播种' : '未播种'}</p>
              </div>
            </div>
            {!hasMaster && failedRun && <AlertTriangle className="w-5 h-5 text-red-400" />}
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 flex items-center justify-between">
            <div className="flex items-center gap-3">
              {hasChapter1 ? (
                <CheckCircle className="w-5 h-5 text-green-400" />
              ) : (
                <XCircle className="w-5 h-5 text-gray-500" />
              )}
              <div>
                <p className="text-white font-medium">CHAPTER_1</p>
                <p className="text-gray-400 text-xs">{hasChapter1 ? '已播种' : '未播种'}</p>
              </div>
            </div>
            {!hasChapter1 && failedRun && <AlertTriangle className="w-5 h-5 text-red-400" />}
          </CardContent>
        </Card>
      </div>

      {(!hasMaster || !hasChapter1) && failedRun && (
        <Card className="border-red-500/30">
          <CardContent className="p-4">
            <div className="flex items-start gap-3">
              <AlertTriangle className="w-5 h-5 text-red-400 flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <p className="text-white font-medium">Genesis 运行失败导致合同缺失</p>
                <p className="text-gray-400 text-xs mt-1">
                  失败运行 ID: {failedRun.id} · 非致命错误: {failedRunErrorCounts.errors} 严重 /{' '}
                  {failedRunErrorCounts.warnings} 警告
                </p>
                {failedRun.error_message && (
                  <p className="text-red-300 text-xs mt-2">{failedRun.error_message}</p>
                )}
                {failedRunErrors.length > 0 && (
                  <ul className="mt-3 space-y-1">
                    {failedRunErrors.slice(0, 5).map((err, idx) => (
                      <li key={idx} className="text-xs text-gray-300">
                        <span
                          className={err.severity === 'error' ? 'text-red-400' : 'text-yellow-400'}
                        >
                          [{err.severity}]
                        </span>{' '}
                        {err.step}: {err.message}
                      </li>
                    ))}
                  </ul>
                )}
                <div className="flex gap-2 mt-4">
                  <Button size="sm" variant="ghost" onClick={() => setCurrentView('logs')}>
                    <ScrollText className="w-3.5 h-3.5 mr-1" />
                    查看日志
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setCurrentView('dashboard')}>
                    <LayoutDashboard className="w-3.5 h-3.5 mr-1" />
                    Genesis 面板
                  </Button>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-white flex items-center gap-2">
                <FileText className="w-5 h-5 text-cinema-gold" />
                合同树
              </h3>
              {!hasMaster && (
                <Button size="sm" onClick={handleCreateMaster} disabled={isLoading}>
                  <Plus className="w-4 h-4 mr-1" />
                  生成世界观合同
                </Button>
              )}
            </div>
            {contractTree?.master_setting ? (
              <div className="space-y-3">
                <div className="p-3 bg-cinema-800 rounded-lg">
                  <p className="text-sm text-gray-400">MASTER_SETTING</p>
                  <p className="text-white text-sm mt-1">
                    {contractTree.master_setting.contract_json.slice(0, 200)}...
                  </p>
                </div>
                <p className="text-sm text-gray-400">
                  章节合同: {Object.keys(contractTree.chapters).length} 个
                </p>
              </div>
            ) : (
              <p className="text-gray-500 text-sm">暂无合同，请先创建 MASTER_SETTING</p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-white">运行时合同</h3>
              <Button size="sm" onClick={handleCreateChapter} disabled={isLoading}>
                <Plus className="w-4 h-4 mr-1" />
                生成章节合同
              </Button>
            </div>
            <div className="flex items-center gap-2 mb-4">
              <input
                type="number"
                value={selectedChapter}
                onChange={e => onChapterChange(parseInt(e.target.value) || 1)}
                className="bg-cinema-800 border border-cinema-700 rounded px-3 py-1 text-white text-sm w-20"
                min={1}
              />
              <Button size="sm" onClick={loadContracts} disabled={isLoading}>
                加载
              </Button>
            </div>
            {runtimeContract ? (
              <div className="space-y-2 text-sm">
                <p className="text-gray-400">
                  核心基调:{' '}
                  {JSON.parse(runtimeContract.master_setting.contract_json).core_tone || 'N/A'}
                </p>
                <p className="text-gray-400">
                  体裁: {JSON.parse(runtimeContract.master_setting.contract_json).genre || 'N/A'}
                </p>
                {runtimeContract.chapter_contract && (
                  <p className="text-gray-400">
                    章节目标:{' '}
                    {JSON.parse(runtimeContract.chapter_contract.contract_json).chapter_directive
                      ?.goal || 'N/A'}
                  </p>
                )}
              </div>
            ) : (
              <p className="text-gray-500 text-sm">点击加载查看运行时合同</p>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
