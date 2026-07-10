import { Navbar } from './components/Navbar';
import { Hero } from './components/Hero';
import { ValueProp } from './components/ValueProp';
import { PainPoints } from './components/PainPoints';
import { BackstageFrontstage } from './components/BackstageFrontstage';
import { Genesis } from './components/Genesis';
import { TimeSliced } from './components/TimeSliced';
import { WhyStoryForge } from './components/WhyStoryForge';
import { Features } from './components/Features';
import { QuickStart } from './components/QuickStart';
import { DownloadCTA } from './components/DownloadCTA';
import { Footer } from './components/Footer';

export default function App() {
  return (
    <>
      <Navbar />
      <main>
        <Hero />
        <ValueProp />
        <PainPoints />
        <BackstageFrontstage />
        <Genesis />
        <TimeSliced />
        <WhyStoryForge />
        <Features />
        <QuickStart />
        <DownloadCTA />
      </main>
      <Footer />
    </>
  );
}
