import { Navbar } from "./components/Navbar";
import { Hero } from "./components/Hero";
import { TrioSection } from "./components/TrioSection";
import { LoopSection } from "./components/LoopSection";
import { CraftSection } from "./components/CraftSection";
import { ScreensSection } from "./components/ScreensSection";
import { QuickStart } from "./components/QuickStart";
import { DownloadCTA } from "./components/DownloadCTA";
import { Footer } from "./components/Footer";

export default function App() {
  return (
    <>
      <Navbar />
      <main>
        <Hero />
        <TrioSection />
        <LoopSection />
        <CraftSection />
        <ScreensSection />
        <QuickStart />
        <DownloadCTA />
      </main>
      <Footer />
    </>
  );
}
