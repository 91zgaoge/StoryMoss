import { Navbar } from './components/Navbar';
import { Hero } from './components/Hero';
import { PainPoints } from './components/PainPoints';
import { Solution } from './components/Solution';
import { Features } from './components/Features';
import { TimeSliced } from './components/TimeSliced';
import { QuickStart } from './components/QuickStart';
import { DownloadCTA } from './components/DownloadCTA';
import { Footer } from './components/Footer';

export default function App() {
  return (
    <>
      <Navbar />
      <main>
        <Hero />
        <PainPoints />
        <Solution />
        <Features />
        <TimeSliced />
        <QuickStart />
        <DownloadCTA />
      </main>
      <Footer />
    </>
  );
}
